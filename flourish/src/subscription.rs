use std::{marker::PhantomData, pin::Pin, sync::Arc};

use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef};

use crate::{
    raw::{new_raw_unsubscribed_subscription_with_runtime, pull_subscription},
    SignalGuard, Source,
};

#[must_use = "Subscriptions are cancelled when dropped."]
pub struct Subscription<
    'a,
    T: 'a + Send + ?Sized,
    SR: 'a + ?Sized + SignalRuntimeRef = GlobalSignalRuntime,
> {
    source: *const (dyn 'a + Source<SR, Value = T>),
    _phantom: PhantomData<Pin<Arc<dyn 'a + Source<SR, Value = T>>>>,
}

impl<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> Drop
    for Subscription<'a, T, SR>
{
    fn drop(&mut self) {
        unsafe { Arc::decrement_strong_count(self.source) }
    }
}

//TODO: Implementations
pub struct SubscriptionGuard<'a, T>(SignalGuard<'a, T>);

/// See [rust-lang#98931](https://github.com/rust-lang/rust/issues/98931).
impl<'a, T: 'a + Send + ?Sized> Subscription<'a, T> {
    pub fn new<F: 'a + Send + FnMut() -> T>(f: F) -> Self
    where
        T: Sized,
    {
        Self::with_runtime(f, GlobalSignalRuntime)
    }
}

impl<'a, T: 'a + Send + ?Sized, SR: SignalRuntimeRef> Subscription<'a, T, SR> {
    pub fn with_runtime<F: 'a + Send + FnMut() -> T>(f: F, runtime: SR) -> Self
    where
        T: Sized,
    {
        let arc = Arc::pin(new_raw_unsubscribed_subscription_with_runtime(f, runtime));
        pull_subscription(arc.as_ref());
        Self {
            source: unsafe { Arc::into_raw(Pin::into_inner_unchecked(arc)) },
            _phantom: PhantomData,
        }
    }

    pub fn as_source(&self) -> Pin<&(dyn 'a + Source<SR, Value = T>)> {
        unsafe { Pin::new_unchecked(&*self.source) }
    }
}
