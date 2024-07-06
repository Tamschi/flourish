use std::{borrow::Borrow, fmt::Debug, marker::PhantomData, mem, pin::Pin, sync::Arc};

use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef, Update};

use crate::{
    raw::{computed, computed_uncached, computed_uncached_mut, folded, merged},
    traits::Subscribable,
    SourcePin, SubscriptionSR,
};

/// Type inference helper alias for [`SignalSR`] (using [`GlobalSignalRuntime`]).
pub type Signal<'a, T> = SignalSR<'a, T, GlobalSignalRuntime>;

/// A largely type-erased signal handle that is all of [`Clone`], [`Send`], [`Sync`] and [`Unpin`].
///
/// You can [`Borrow`] this handle into a reference without indirection that is [`ToOwned`].
///
/// To access values, import [`SourcePin`].
///
/// Signals are not evaluated unless they are subscribed-to (or on demand if if not current).  
/// Uncached signals are instead evaluated on direct demand **only** (but still communicate subscriptions and invalidation).
#[derive(Clone)]
pub struct SignalSR<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> {
    pub(super) source: Pin<Arc<dyn 'a + Subscribable<SR, Value = T>>>,
}

unsafe impl<'a, T: Send + ?Sized, SR: ?Sized + SignalRuntimeRef> Send for SignalSR<'a, T, SR> {}
unsafe impl<'a, T: Send + ?Sized, SR: ?Sized + SignalRuntimeRef> Sync for SignalSR<'a, T, SR> {}

impl<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> SignalSR<'a, T, SR> {
    /// Creates a new [`SignalSR`] from the provided raw [`Subscribable`].
    pub fn new(source: impl 'a + Subscribable<SR, Value = T>) -> Self {
        SignalSR {
            source: Arc::pin(source),
        }
    }

    pub fn try_subscribe(mut self) -> Result<SubscriptionSR<'a, T, SR>, Self> {
        //TODO: This could be more efficient.
        match Arc::get_mut(unsafe {
            mem::transmute::<
                &'_ mut Pin<Arc<dyn 'a + Subscribable<SR, Value = T>>>,
                &'_ mut Arc<dyn 'a + Subscribable<SR, Value = T>>,
            >(&mut self.source)
        }) {
            Some(_) => {
                let source = Pin::clone(&self.source);
                source.as_ref().pull();
                Ok(SubscriptionSR { source })
            }
            None => Err(self),
        }
    }

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
    pub fn computed(f: impl 'a + Send + FnMut() -> T) -> Self
    where
        T: Sized,
        SR: Default,
    {
        Self::new(computed(f, SR::default()))
    }

    /// A simple cached computation.
    ///
    /// Wraps [`computed`](`computed()`).
    pub fn computed_with_runtime(f: impl 'a + Send + FnMut() -> T, runtime: SR) -> Self
    where
        T: Sized,
    {
        Self::new(computed(f, runtime))
    }

    /// A simple **uncached** computation.
    ///
    /// Wraps [`computed_uncached`](`computed_uncached()`).
    pub fn computed_uncached(f: impl 'a + Send + Sync + Fn() -> T) -> Self
    where
        T: Sized,
        SR: Default,
    {
        Self::computed_uncached_with_runtime(f, SR::default())
    }

    /// A simple **uncached** computation.
    ///
    /// Wraps [`computed_uncached`](`computed_uncached()`).
    pub fn computed_uncached_with_runtime(f: impl 'a + Send + Sync + Fn() -> T, runtime: SR) -> Self
    where
        T: Sized,
    {
        Self::new(computed_uncached(f, runtime))
    }

    /// A simple **stateful uncached** computation.
    ///
    /// ⚠️ Care must be taken to avoid unexpected behaviour!
    ///
    /// Wraps [`computed_uncached_mut`](`computed_uncached_mut()`).
    pub fn computed_uncached_mut(f: impl 'a + Send + FnMut() -> T) -> Self
    where
        T: Sized,
        SR: Default,
    {
        Self::computed_uncached_mut_with_runtime(f, SR::default())
    }

    /// A simple **stateful uncached** computation.
    ///
    /// ⚠️ Care must be taken to avoid unexpected behaviour!
    ///
    /// Wraps [`computed_uncached_mut`](`computed_uncached_mut()`).
    pub fn computed_uncached_mut_with_runtime(f: impl 'a + Send + FnMut() -> T, runtime: SR) -> Self
    where
        T: Sized,
    {
        Self::new(computed_uncached_mut(f, runtime))
    }

    /// The closure mutates the value and can choose to [`Halt`](`Update::Halt`) propagation.
    ///
    /// Wraps [`folded`](`folded()`).
    pub fn folded(init: T, f: impl 'a + Send + FnMut(&mut T) -> Update) -> Self
    where
        T: Sized,
        SR: Default,
    {
        Self::folded_with_runtime(init, f, SR::default())
    }

    /// The closure mutates the value and can choose to [`Halt`](`Update::Halt`) propagation.
    ///
    /// Wraps [`folded`](`folded()`).
    pub fn folded_with_runtime(
        init: T,
        f: impl 'a + Send + FnMut(&mut T) -> Update,
        runtime: SR,
    ) -> Self
    where
        T: Sized,
    {
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
        T: Sized,
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
    ) -> Self
    where
        T: Sized,
    {
        Self::new(merged(select, merge, runtime))
    }
}

impl<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> SourcePin<SR>
    for SignalSR<'a, T, SR>
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

/// A free [`SignalSR`] or [`SubscriptionSR`] borrow that can be easily copied.
#[repr(transparent)]
#[derive(Debug)]
pub struct SignalRef<'r, 'a, T: 'a + Send + ?Sized, SR: ?Sized + SignalRuntimeRef> {
    pub(crate) source: *const (dyn 'a + Subscribable<SR, Value = T>),
    _phantom: PhantomData<(&'r (dyn 'a + Subscribable<SR, Value = T>), SR)>,
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

impl<'r, 'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef>
    Borrow<SignalRef<'r, 'a, T, SR>> for SignalSR<'a, T, SR>
{
    fn borrow(&self) -> &SignalRef<'r, 'a, T, SR> {
        unsafe { &*((self as *const Self).cast()) }
    }
}

impl<'r, 'a, T: Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> SourcePin<SR>
    for SignalRef<'r, 'a, T, SR>
{
    type Value = T;

    //SAFETY: `self.source` is a payload pointer that's valid for at least 'r.

    fn touch(&self) {
        unsafe { Pin::new_unchecked(&*self.source) }.touch()
    }

    fn get_clone(&self) -> Self::Value
    where
        Self::Value: Sync + Clone,
    {
        unsafe { Pin::new_unchecked(&*self.source) }.get_clone()
    }

    fn get_clone_exclusive(&self) -> Self::Value
    where
        Self::Value: Clone,
    {
        unsafe { Pin::new_unchecked(&*self.source) }.get_clone_exclusive()
    }

    fn read<'s>(&'s self) -> Box<dyn 's + Borrow<Self::Value>>
    where
        Self::Value: 's + Sync,
    {
        unsafe { Pin::new_unchecked(&*self.source) }.read()
    }

    fn read_exclusive<'s>(&'s self) -> Box<dyn 's + Borrow<Self::Value>> {
        unsafe { Pin::new_unchecked(&*self.source) }.read_exclusive()
    }

    fn clone_runtime_ref(&self) -> SR
    where
        SR: Sized,
    {
        unsafe { Pin::new_unchecked(&*self.source) }.clone_runtime_ref()
    }

    fn get(&self) -> Self::Value
    where
        Self::Value: Sync + Copy,
    {
        unsafe { Pin::new_unchecked(&*self.source) }.get()
    }

    fn get_exclusive(&self) -> Self::Value
    where
        Self::Value: Copy,
    {
        unsafe { Pin::new_unchecked(&*self.source) }.get_exclusive()
    }
}
