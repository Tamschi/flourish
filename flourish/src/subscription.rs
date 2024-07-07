use std::{borrow::Borrow, fmt::Debug, marker::PhantomData, pin::Pin, sync::Arc};

use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef, Update};

use crate::{
    raw::{computed, folded, merged},
    traits::Subscribable,
    SignalRef, SignalSR, SourcePin,
};

/// Type inference helper alias for [`SubscriptionSR`] (using [`GlobalSignalRuntime`]).
pub type Subscription<'a, T> = SubscriptionSR<'a, T, GlobalSignalRuntime>;

/// Inherently-subscribed version of [`SignalSR`].  
/// Can be directly constructed but also converted to and fro.
#[must_use = "Subscriptions are cancelled when dropped."]
pub struct SubscriptionSR<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> {
    pub(crate) source: Pin<Arc<dyn 'a + Subscribable<SR, Value = T>>>,
}

impl<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> Debug
    for SubscriptionSR<'a, T, SR>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.source.clone_runtime_ref().run_detached(|| {
            f.debug_struct("SubscriptionSR")
                .field(
                    "(value)",
                    &(&*self.source.as_ref().read_exclusive()).borrow(),
                )
                .finish_non_exhaustive()
        })
    }
}

unsafe impl<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> Send
    for SubscriptionSR<'a, T, SR>
{
}
unsafe impl<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> Sync
    for SubscriptionSR<'a, T, SR>
{
}

impl<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> Drop
    for SubscriptionSR<'a, T, SR>
{
    fn drop(&mut self) {
        self.source.as_ref().unsubscribe();
    }
}

/// See [rust-lang#98931](https://github.com/rust-lang/rust/issues/98931).
impl<'a, T: 'a + Send + ?Sized, SR: SignalRuntimeRef> SubscriptionSR<'a, T, SR> {
    /// Constructs a new [`SubscriptionSR`] from the given "raw" [`Subscribable`].
    ///
    /// The subscribable is [`pull`](`Subscribable::pull`)ed once.
    pub fn new<S: 'a + Subscribable<SR, Value = T>>(source: S) -> Self {
        source.clone_runtime_ref().run_detached(|| {
            let arc = Arc::pin(source);
            arc.as_ref().pull();
            Self { source: arc }
        })
    }

    /// Cheaply borrows this [`SubscriptionSR`] as [`SignalRef`], which is [`Clone`].
    pub fn as_ref(&self) -> SignalRef<'_, 'a, T, SR> {
        SignalRef {
            source: {
                let ptr =
                    Arc::into_raw(unsafe { Pin::into_inner_unchecked(Pin::clone(&self.source)) });
                unsafe { Arc::decrement_strong_count(ptr) };
                ptr
            },
            _phantom: PhantomData,
        }
    }

    /// Unsubscribes the [`SubscriptionSR`], turning it into a [`SignalSR`] in the process.
    ///
    /// The underlying [`Source`](`crate::raw::Source`) may remain effectively subscribed due to subscribed dependencies.
    #[must_use = "Use `drop(self)` instead of converting first. The effect is the same."]
    pub fn unsubscribe(self) -> SignalSR<'a, T, SR> {
        //FIXME: This could avoid refcounting up and down and the associated memory barriers.
        SignalSR {
            source: Pin::clone(&self.source),
        }
    } // Implicit drop(self) unsubscribes.

    /// Cheaply clones this handle into a [`SignalSR`].
    ///
    /// Only one handle can own the inherent subscription of the managed [`Subscribable`].
    #[must_use = "Pure function."]
    pub fn to_signal(self) -> SignalSR<'a, T, SR> {
        SignalSR {
            source: Pin::clone(&self.source),
        }
    }
}

/// Secondary constructors.
///
/// # Omissions
///
/// The "uncached" versions of [`computed`](`computed()`) are intentionally not wrapped here,
/// as their behaviour may be unexpected at first glance.
///
/// You can still easily construct them as [`SignalSR`] and subscribe afterwards:
///
/// ```
/// use flourish::Signal;
///
/// // The closure runs once on subscription, but not to refresh `sub`!
/// // It re-runs with each access of its value through `SourcePin`, instead.
/// let sub = Signal::computed_uncached(|| ())
///     .try_subscribe()
///     .expect("contextually infallible");
/// ```
impl<'a, T: 'a + Send, SR: SignalRuntimeRef> SubscriptionSR<'a, T, SR> {
    /// A simple cached computation.
    ///
    /// Wraps [`computed`](`computed()`).
    pub fn computed(f: impl 'a + Send + FnMut() -> T) -> Self
    where
        SR: Default,
    {
        Self::new(computed(f, SR::default()))
    }

    /// A simple cached computation.
    ///
    /// Wraps [`computed`](`computed()`).
    pub fn computed_with_runtime(f: impl 'a + Send + FnMut() -> T, runtime: SR) -> Self {
        Self::new(computed(f, runtime))
    }

    /// The closure mutates the value and can choose to [`Halt`](`Update::Halt`) propagation.
    ///
    /// Wraps [`folded`](`folded()`).
    pub fn folded(init: T, f: impl 'a + Send + FnMut(&mut T) -> Update) -> Self
    where
        SR: Default,
    {
        Self::new(folded(init, f, SR::default()))
    }

    /// The closure mutates the value and can choose to [`Halt`](`Update::Halt`) propagation.
    ///
    /// Wraps [`folded`](`folded()`).
    pub fn folded_with_runtime(
        init: T,
        f: impl 'a + Send + FnMut(&mut T) -> Update,
        runtime: SR,
    ) -> Self {
        Self::new(folded(init, f, runtime))
    }

    /// `select` computes each value, `merge` updates current with next and can choose to [`Halt`](`Update::Halt`) propagation.
    /// Dependencies are detected across both closures.
    ///
    /// Wraps [`merged`](`merged()`).
    pub fn merged(
        select: impl 'a + Send + FnMut() -> T,
        merge: impl 'a + Send + FnMut(&mut T, T) -> Update,
    ) -> Self
    where
        SR: Default,
    {
        Self::new(merged(select, merge, SR::default()))
    }

    /// `select` computes each value, `merge` updates current with next and can choose to [`Halt`](`Update::Halt`) propagation.
    /// Dependencies are detected across both closures.
    ///
    /// Wraps [`merged`](`merged()`).
    pub fn merged_with_runtime(
        select: impl 'a + Send + FnMut() -> T,
        merge: impl 'a + Send + FnMut(&mut T, T) -> Update,
        runtime: SR,
    ) -> Self {
        Self::new(merged(select, merge, runtime))
    }
}

impl<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> SourcePin<SR>
    for SubscriptionSR<'a, T, SR>
{
    type Value = T;

    fn touch(&self) {
        self.source.as_ref().touch()
    }

    fn get_clone(&self) -> Self::Value
    where
        Self::Value: Sync + Clone,
    {
        self.source.as_ref().get_clone()
    }

    fn get_clone_exclusive(&self) -> Self::Value
    where
        Self::Value: Clone,
    {
        self.source.as_ref().get_clone_exclusive()
    }

    fn read<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Value>>
    where
        Self::Value: 'r + Sync,
    {
        self.source.as_ref().read()
    }

    fn read_exclusive<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Value>> {
        self.source.as_ref().read_exclusive()
    }

    fn clone_runtime_ref(&self) -> SR
    where
        SR: Sized,
    {
        self.source.as_ref().clone_runtime_ref()
    }
}

// TODO: `unsubscribe(self)` to convert into `SignalSR`, `to_signal(&self)`.
