use std::{marker::PhantomData, mem, pin::Pin, sync::Arc};

use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef};

use crate::{
    SignalGuard, Source,
    __::{__new_raw_unsubscribed_subscription_with_runtime, __pull_subscription},
};

#[must_use = "Subscriptions are cancelled when dropped."]
pub struct Subscription<T: Send + ?Sized, SR: ?Sized + SignalRuntimeRef = GlobalSignalRuntime> {
    source: Pin<*const dyn Source<SR, Value = T>>,
    _phantom: PhantomData<(Arc<dyn Source<SR, Value = T>>, SR)>,
}

//TODO: Implementations
pub struct SubscriptionGuard<'a, T>(SignalGuard<'a, T>);

/// See [rust-lang#98931](https://github.com/rust-lang/rust/issues/98931).
impl<T: Send + ?Sized> Subscription<T> {
    pub fn new<F: Send + FnMut() -> T>(f: F) -> Self
    where
        T: Sized,
    {
        Self::with_runtime(f, GlobalSignalRuntime)
    }
}

impl<T: Send + ?Sized, SR: SignalRuntimeRef> Subscription<T, SR> {
    pub fn with_runtime<F: Send + FnMut() -> T>(f: F, runtime: SR) -> Self
    where
        T: Sized,
    {
        let arc = Arc::pin(__new_raw_unsubscribed_subscription_with_runtime(f, runtime));
        __pull_subscription(arc.as_ref());
        Self {
            source: unsafe {
                mem::transmute::<
                    *const dyn Source<SR, Value = T>,
                    Pin<*const dyn Source<SR, Value = T>>,
                >(Arc::into_raw(Pin::into_inner_unchecked(arc)))
            },
            _phantom: PhantomData,
        }
    }

    pub fn as_source(&self) -> Pin<&(dyn Source<SR, Value = T>)> {
        unsafe {
            Pin::new_unchecked(&*mem::transmute::<
                Pin<*const dyn Source<SR, Value = T>>,
                *const dyn Source<SR, Value = T>,
            >(self.source))
        }
    }
}
