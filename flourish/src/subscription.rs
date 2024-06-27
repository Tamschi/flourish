use std::{marker::PhantomData, pin::Pin, sync::Arc};

use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef};

use crate::{
    raw::{new_raw_unsubscribed_subscription_with_runtime, pull_subscription},
    SignalGuard, Source,
};

#[must_use = "Subscriptions are cancelled when dropped."]
pub struct Subscription<
    'a,
    T: 'a + Send + ?Sized + Clone,
    SR: 'a + ?Sized + SignalRuntimeRef = GlobalSignalRuntime,
> {
    source: *const (dyn 'a + Source<SR, Value = T>),
    _phantom: PhantomData<Pin<Arc<dyn 'a + Source<SR, Value = T>>>>,
}

impl<'a, T: 'a + Send + ?Sized + Clone, SR: 'a + ?Sized + SignalRuntimeRef> Drop
    for Subscription<'a, T, SR>
{
    fn drop(&mut self) {
        unsafe { Arc::decrement_strong_count(self.source) }
    }
}

//TODO: Implementations
pub struct SubscriptionGuard<'a, T>(SignalGuard<'a, T>);

/// See [rust-lang#98931](https://github.com/rust-lang/rust/issues/98931).
impl<'a, T: 'a + Send + ?Sized + Clone, SR: SignalRuntimeRef> Subscription<'a, T, SR> {
    pub fn new<S: 'a + Source<SR, Value = T>>(source: S) -> Self
    where
        T: Sized,
    {
        {
            let runtime = source.clone_runtime_ref();
            let arc = Arc::pin(new_raw_unsubscribed_subscription_with_runtime(
                source, runtime,
            ));
            pull_subscription(arc.as_ref());
            Self {
                source: unsafe { Arc::into_raw(Pin::into_inner_unchecked(arc)) },
                _phantom: PhantomData,
            }
        }
    }
}
