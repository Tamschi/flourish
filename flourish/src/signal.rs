use std::{
    borrow::Borrow,
    fmt::{self, Debug, Formatter},
    marker::PhantomData,
    pin::Pin,
    sync::Arc,
};

use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef, Update};

use crate::{
    raw::{computed, computed_uncached, computed_uncached_mut, debounced, folded, merged},
    traits::Subscribable,
    SourcePin, SubscriptionSR,
};

/// Type inference helper alias for [`SignalSR`] (using [`GlobalSignalRuntime`]).
pub type Signal<'a, T> = SignalSR<'a, T, GlobalSignalRuntime>;

/// A largely type-erased signal handle that is all of [`Clone`], [`Send`], [`Sync`] and [`Unpin`].
///
/// To access values, import [`SourcePin`].
///
/// Signals are not evaluated unless they are subscribed-to (or on demand if if not current).  
/// Uncached signals are instead evaluated on direct demand **only** (but still communicate subscriptions and invalidation).
pub struct SignalSR<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> {
    pub(super) source: Pin<Arc<dyn 'a + Subscribable<SR, Output = T>>>,
}

impl<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> Clone for SignalSR<'a, T, SR> {
    fn clone(&self) -> Self {
        Self {
            source: self.source.clone(),
        }
    }
}

impl<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> Debug for SignalSR<'a, T, SR>
where
    T: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.source.clone_runtime_ref().run_detached(|| {
            f.debug_struct("SignalSR")
                .field(
                    "(value)",
                    &(&*self.source.as_ref().read_exclusive()).borrow(),
                )
                .finish_non_exhaustive()
        })
    }
}

unsafe impl<'a, T: Send + ?Sized, SR: ?Sized + SignalRuntimeRef> Send for SignalSR<'a, T, SR> {}
unsafe impl<'a, T: Send + ?Sized, SR: ?Sized + SignalRuntimeRef> Sync for SignalSR<'a, T, SR> {}

impl<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> SignalSR<'a, T, SR> {
    /// Creates a new [`SignalSR`] from the provided raw [`Subscribable`].
    pub fn new(source: impl 'a + Subscribable<SR, Output = T>) -> Self {
        SignalSR {
            source: Arc::pin(source),
        }
    }

    /// Cheaply borrows this [`SignalSR`] as [`SignalRef`], which is [`Clone`].
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

    pub fn try_subscribe(self) -> Result<SubscriptionSR<'a, T, SR>, Self> {
        if (|| self.source.as_ref().subscribe_inherently().is_some())() {
            Ok(SubscriptionSR {
                source: self.source,
            })
        } else {
            Err(self)
        }
    }

    /// First calls [`self.try_subscribe()`](`SignalSR::try_subscribe`) and, iff that fails,
    /// falls back to constructing a computed (cached) subscription from `make_fn_pin(self)`'s output.
    pub fn subscribe_or_computed<FnPin: 'a + Send + FnMut() -> T>(
        self,
        make_fn_pin: impl FnOnce(Self) -> FnPin,
    ) -> SubscriptionSR<'a, T, SR>
    where
        T: Sized,
    {
        self.try_subscribe().unwrap_or_else(move |this| {
            let runtime = this.clone_runtime_ref();
            SubscriptionSR::computed_with_runtime(make_fn_pin(this), runtime)
        })
    }
}

/// Secondary constructors.
impl<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> SignalSR<'a, T, SR> {
    /// A simple cached computation.
    ///
    /// Wraps [`computed`](`computed()`).
    pub fn computed(fn_pin: impl 'a + Send + FnMut() -> T) -> Self
    where
        T: Sized,
        SR: Default,
    {
        Self::new(computed(fn_pin, SR::default()))
    }

    /// A simple cached computation.
    ///
    /// Wraps [`computed`](`computed()`).
    pub fn computed_with_runtime(fn_pin: impl 'a + Send + FnMut() -> T, runtime: SR) -> Self
    where
        T: Sized,
    {
        Self::new(computed(fn_pin, runtime))
    }

    /// A simple cached computation.
    ///
    /// Doesn't update its cache or propagate iff the new result is equal.
    ///
    /// Wraps [`debounced`](`debounced()`).
    pub fn debounced(fn_pin: impl 'a + Send + FnMut() -> T) -> Self
    where
        T: Sized + PartialEq,
        SR: Default,
    {
        Self::new(debounced(fn_pin, SR::default()))
    }

    /// A simple cached computation.
    ///
    /// Doesn't update its cache or propagate iff the new result is equal.
    ///
    /// Wraps [`debounced`](`debounced()`).
    pub fn debounced_with_runtime(fn_pin: impl 'a + Send + FnMut() -> T, runtime: SR) -> Self
    where
        T: Sized + PartialEq,
    {
        Self::new(debounced(fn_pin, runtime))
    }

    /// A simple **uncached** computation.
    ///
    /// Wraps [`computed_uncached`](`computed_uncached()`).
    pub fn computed_uncached(fn_pin: impl 'a + Send + Sync + Fn() -> T) -> Self
    where
        T: Sized,
        SR: Default,
    {
        Self::computed_uncached_with_runtime(fn_pin, SR::default())
    }

    /// A simple **uncached** computation.
    ///
    /// Wraps [`computed_uncached`](`computed_uncached()`).
    pub fn computed_uncached_with_runtime(
        fn_pin: impl 'a + Send + Sync + Fn() -> T,
        runtime: SR,
    ) -> Self
    where
        T: Sized,
    {
        Self::new(computed_uncached(fn_pin, runtime))
    }

    /// A simple **stateful uncached** computation.
    ///
    /// ⚠️ Care must be taken to avoid unexpected behaviour!
    ///
    /// Wraps [`computed_uncached_mut`](`computed_uncached_mut()`).
    pub fn computed_uncached_mut(fn_pin: impl 'a + Send + FnMut() -> T) -> Self
    where
        T: Sized,
        SR: Default,
    {
        Self::computed_uncached_mut_with_runtime(fn_pin, SR::default())
    }

    /// A simple **stateful uncached** computation.
    ///
    /// ⚠️ Care must be taken to avoid unexpected behaviour!
    ///
    /// Wraps [`computed_uncached_mut`](`computed_uncached_mut()`).
    pub fn computed_uncached_mut_with_runtime(
        fn_pin: impl 'a + Send + FnMut() -> T,
        runtime: SR,
    ) -> Self
    where
        T: Sized,
    {
        Self::new(computed_uncached_mut(fn_pin, runtime))
    }

    /// The closure mutates the value and can choose to [`Halt`](`Update::Halt`) propagation.
    ///
    /// Wraps [`folded`](`folded()`).
    pub fn folded(init: T, fold_fn_pin: impl 'a + Send + FnMut(&mut T) -> Update) -> Self
    where
        T: Sized,
        SR: Default,
    {
        Self::folded_with_runtime(init, fold_fn_pin, SR::default())
    }

    /// The closure mutates the value and can choose to [`Halt`](`Update::Halt`) propagation.
    ///
    /// Wraps [`folded`](`folded()`).
    pub fn folded_with_runtime(
        init: T,
        fold_fn_pin: impl 'a + Send + FnMut(&mut T) -> Update,
        runtime: SR,
    ) -> Self
    where
        T: Sized,
    {
        Self::new(folded(init, fold_fn_pin, runtime))
    }

    /// `select_fn_pin` computes each value, `merge_fn_pin` updates current with next and can choose to [`Halt`](`Update::Halt`) propagation.
    /// Dependencies are detected across both closures.
    ///
    /// Wraps [`merged`](`merged()`).
    pub fn merged(
        select_fn_pin: impl 'a + Send + FnMut() -> T,
        merge_fn_pin: impl 'a + Send + FnMut(&mut T, T) -> Update,
    ) -> Self
    where
        T: Sized,
        SR: Default,
    {
        Self::new(merged(select_fn_pin, merge_fn_pin, SR::default()))
    }

    /// `select_fn_pin` computes each value, `merge_fn_pin` updates current with next and can choose to [`Halt`](`Update::Halt`) propagation.
    /// Dependencies are detected across both closures.
    ///
    /// Wraps [`merged`](`merged()`).
    pub fn merged_with_runtime(
        select_fn_pin: impl 'a + Send + FnMut() -> T,
        merge_fn_pin: impl 'a + Send + FnMut(&mut T, T) -> Update,
        runtime: SR,
    ) -> Self
    where
        T: Sized,
    {
        Self::new(merged(select_fn_pin, merge_fn_pin, runtime))
    }
}

impl<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> SourcePin<SR>
    for SignalSR<'a, T, SR>
{
    type Output = T;

    fn touch(&self) {
        self.source.as_ref().touch()
    }

    fn get_clone(&self) -> Self::Output
    where
        Self::Output: Sync + Clone,
    {
        self.source.as_ref().get_clone()
    }

    fn get_clone_exclusive(&self) -> Self::Output
    where
        Self::Output: Clone,
    {
        self.source.as_ref().get_clone_exclusive()
    }

    fn read<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Output>>
    where
        Self::Output: 'r + Sync,
    {
        self.source.as_ref().read()
    }

    fn read_exclusive<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Output>> {
        self.source.as_ref().read_exclusive()
    }

    fn clone_runtime_ref(&self) -> SR
    where
        SR: Sized,
    {
        self.source.as_ref().clone_runtime_ref()
    }
}

/// A very cheap [`SignalSR`] or [`SubscriptionSR`] borrow that's [`Copy`].
///
/// Can be cloned into an additional [`SignalSR`] or subscribed to.
#[derive(Debug)]
pub struct SignalRef<'r, 'a, T: 'a + Send + ?Sized, SR: ?Sized + SignalRuntimeRef> {
    pub(crate) source: *const (dyn 'a + Subscribable<SR, Output = T>),
    pub(crate) _phantom: PhantomData<(&'r (dyn 'a + Subscribable<SR, Output = T>), SR)>,
}

impl<'r, 'a, T: 'a + Send + ?Sized, SR: ?Sized + SignalRuntimeRef> SignalRef<'r, 'a, T, SR> {
    /// Cheaply creates an additional [`SignalSR`] managing the same [`Subscribable`].
    pub fn to_signal(&self) -> SignalSR<'a, T, SR> {
        SignalSR {
            source: unsafe {
                Arc::increment_strong_count(self.source);
                Pin::new_unchecked(Arc::from_raw(self.source))
            },
        }
    }

    /// Creates a computed (cached) [`SubscriptionSR`] based on this [`SignalRef`].
    ///
    /// This is a shortcut past `self.to_signal().subscribe_or_computed(make_fn_pin)`.
    /// (This method may be slightly more efficient.)
    pub fn subscribe_computed<FnPin: 'a + Send + FnMut() -> T>(
        &self,
        make_fn_pin: impl FnOnce(SignalSR<'a, T, SR>) -> FnPin,
    ) -> SubscriptionSR<'a, T, SR>
    where
        T: Sized,
    {
        SubscriptionSR::computed_with_runtime(
            make_fn_pin(self.to_signal()),
            unsafe { Pin::new_unchecked(&*self.source) }.clone_runtime_ref(),
        )
    }
}

impl<'r, 'a, T: 'a + Send + ?Sized, SR: ?Sized + SignalRuntimeRef> Clone
    for SignalRef<'r, 'a, T, SR>
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<'r, 'a, T: 'a + Send + ?Sized, SR: ?Sized + SignalRuntimeRef> Copy
    for SignalRef<'r, 'a, T, SR>
{
}

unsafe impl<'r, 'a, T: 'a + Send + ?Sized, SR: ?Sized + SignalRuntimeRef> Send
    for SignalRef<'r, 'a, T, SR>
{
    // SAFETY: The [`Subscribable`] used internally requires both [`Send`] and [`Sync`] of the underlying object.
}

unsafe impl<'r, 'a, T: 'a + Send + ?Sized, SR: ?Sized + SignalRuntimeRef> Sync
    for SignalRef<'r, 'a, T, SR>
{
    // SAFETY: The [`Subscribable`] used internally requires both [`Send`] and [`Sync`] of the underlying object.
}

impl<'r, 'a, T: Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> SourcePin<SR>
    for SignalRef<'r, 'a, T, SR>
{
    type Output = T;

    //SAFETY: `self.source` is a payload pointer that's valid for at least 'r.

    fn touch(&self) {
        unsafe { Pin::new_unchecked(&*self.source) }.touch()
    }

    fn get_clone(&self) -> Self::Output
    where
        Self::Output: Sync + Clone,
    {
        unsafe { Pin::new_unchecked(&*self.source) }.get_clone()
    }

    fn get_clone_exclusive(&self) -> Self::Output
    where
        Self::Output: Clone,
    {
        unsafe { Pin::new_unchecked(&*self.source) }.get_clone_exclusive()
    }

    fn read<'s>(&'s self) -> Box<dyn 's + Borrow<Self::Output>>
    where
        Self::Output: 's + Sync,
    {
        unsafe { Pin::new_unchecked(&*self.source) }.read()
    }

    fn read_exclusive<'s>(&'s self) -> Box<dyn 's + Borrow<Self::Output>> {
        unsafe { Pin::new_unchecked(&*self.source) }.read_exclusive()
    }

    fn clone_runtime_ref(&self) -> SR
    where
        SR: Sized,
    {
        unsafe { Pin::new_unchecked(&*self.source) }.clone_runtime_ref()
    }

    fn get(&self) -> Self::Output
    where
        Self::Output: Sync + Copy,
    {
        unsafe { Pin::new_unchecked(&*self.source) }.get()
    }

    fn get_exclusive(&self) -> Self::Output
    where
        Self::Output: Copy,
    {
        unsafe { Pin::new_unchecked(&*self.source) }.get_exclusive()
    }
}
