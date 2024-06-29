use std::{marker::PhantomData, mem, ops::Deref, pin::Pin, sync::Arc};

use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef, Update};

use crate::{
    raw::{computed, folded, merged, new_raw_unsubscribed_subscription, pull_subscription},
    Source,
};

pub type Subscription<'a, T> = SubscriptionSR<'a, T, GlobalSignalRuntime>;

#[must_use = "Subscriptions are cancelled when dropped."]
pub struct SubscriptionSR<
    'a,
    T: 'a + Send + ?Sized + Clone,
    SR: 'a + ?Sized + SignalRuntimeRef = GlobalSignalRuntime,
> {
    source: *const (dyn 'a + Source<SR, Value = T>),
    _phantom: PhantomData<Pin<Arc<dyn 'a + Source<SR, Value = T>>>>,
}

impl<'a, T: 'a + Send + ?Sized + Clone, SR: 'a + ?Sized + SignalRuntimeRef> Deref
    for SubscriptionSR<'a, T, SR>
{
    type Target = Pin<&'a (dyn 'a + Source<SR, Value = T>)>;

    fn deref(&self) -> &Self::Target {
        unsafe {
            mem::transmute::<
                &*const (dyn 'a + Source<SR, Value = T>),
                &Pin<&'a (dyn 'a + Source<SR, Value = T>)>,
            >(&self.source)
        }
    }
}

impl<'a, T: 'a + Send + ?Sized + Clone, SR: 'a + ?Sized + SignalRuntimeRef> Drop
    for SubscriptionSR<'a, T, SR>
{
    fn drop(&mut self) {
        unsafe { Arc::decrement_strong_count(self.source) }
    }
}

/// See [rust-lang#98931](https://github.com/rust-lang/rust/issues/98931).
impl<'a, T: 'a + Send + ?Sized + Clone, SR: SignalRuntimeRef> SubscriptionSR<'a, T, SR> {
    pub fn new<S: 'a + Source<SR, Value = T>>(source: S) -> Self {
        source.clone_runtime_ref().run_detached(|| {
            let arc = Arc::pin(new_raw_unsubscribed_subscription(source));
            pull_subscription(arc.as_ref());
            Self {
                source: unsafe { Arc::into_raw(Pin::into_inner_unchecked(arc)) },
                _phantom: PhantomData,
            }
        })
    }

    pub fn computed(f: impl 'a + Send + FnMut() -> T) -> Self
    where
        SR: Default,
    {
        Self::new(computed(f, SR::default()))
    }

    pub fn computed_with_runtime(f: impl 'a + Send + FnMut() -> T, runtime: SR) -> Self {
        Self::new(computed(f, runtime))
    }

    /// This is a convenience method. See [`folded`](`folded()`).
    pub fn folded(init: T, f: impl 'a + Send + FnMut(&mut T) -> Update) -> Self
    where
        SR: Default,
    {
        Self::new(folded(init, f, SR::default()))
    }

    /// This is a convenience method. See [`folded`](`folded()`).
    pub fn folded_with_runtime(
        init: T,
        f: impl 'a + Send + FnMut(&mut T) -> Update,
        runtime: SR,
    ) -> Self {
        Self::new(folded(init, f, runtime))
    }

    /// This is a convenience method. See [`merged`](`merged()`).
    pub fn merged(
        select: impl 'a + Send + FnMut() -> T,
        merge: impl 'a + Send + FnMut(&mut T, T) -> Update,
    ) -> Self
    where
        SR: Default,
    {
        Self::new(merged(select, merge, SR::default()))
    }

    /// This is a convenience method. See [`merged`](`merged()`).
    pub fn merged_with_runtime(
        select: impl 'a + Send + FnMut() -> T,
        merge: impl 'a + Send + FnMut(&mut T, T) -> Update,
        runtime: SR,
    ) -> Self {
        Self::new(merged(select, merge, runtime))
    }
}
