use std::{marker::PhantomData, pin::Pin, sync::Arc};

use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef};

use crate::{
    raw::{
        computed, computed_uncached, computed_uncached_mut, new_raw_unsubscribed_subscription,
        pull_subscription,
    },
    AsSource, SignalGuard, Source,
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

impl<'a, T: 'a + Send + ?Sized + Clone, SR: 'a + ?Sized + SignalRuntimeRef> Drop
    for SubscriptionSR<'a, T, SR>
{
    fn drop(&mut self) {
        unsafe { Arc::decrement_strong_count(self.source) }
    }
}

//TODO: Implementations
pub struct SubscriptionGuard<'a, T>(SignalGuard<'a, T>);

/// See [rust-lang#98931](https://github.com/rust-lang/rust/issues/98931).
impl<'a, T: 'a + Send + ?Sized + Clone, SR: SignalRuntimeRef> SubscriptionSR<'a, T, SR> {
    pub fn new<S: 'a + Source<SR, Value = T>>(source: S) -> Self
    where
        T: Sized,
    {
        {
            let arc = Arc::pin(new_raw_unsubscribed_subscription(source));
            pull_subscription(arc.as_ref());
            Self {
                source: unsafe { Arc::into_raw(Pin::into_inner_unchecked(arc)) },
                _phantom: PhantomData,
            }
        }
    }

    pub fn computed(f: impl 'a + Send + FnMut() -> T) -> Self
    where
        T: Send + Sync + Sized + Clone,
        SR: Default,
    {
        Self::new(computed(f, SR::default()))
    }

    pub fn computed_with_runtime(f: impl 'a + Send + FnMut() -> T, runtime: SR) -> Self
    where
        T: Send + Sync + Sized + Clone,
    {
        Self::new(computed(f, runtime))
    }
}

impl<'a, T: 'a + Send + ?Sized + Clone, SR: SignalRuntimeRef> AsSource<'a, SR>
    for SubscriptionSR<'a, T, SR>
{
    type Source = dyn 'a + Source<SR, Value = T>;

    fn as_source(&self) -> Pin<&Self::Source> {
        unsafe { Pin::new_unchecked(&*self.source) }
    }
}
