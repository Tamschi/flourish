use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef};

use crate::{Signal, SignalGuard};

#[must_use = "Subscriptions are cancelled when dropped."]
pub struct Subscription<T: Send + ?Sized, SR: SignalRuntimeRef = GlobalSignalRuntime>(
    Signal<T, SR>,
);

//TODO: Implementations
pub struct SubscriptionGuard<'a, T>(SignalGuard<'a, T>);

impl<T: Send + ?Sized, SR: SignalRuntimeRef> Subscription<T, SR> {
    pub fn new<F: Send + FnMut() -> T>(f: F) -> Self
    where
        T: Sized,
        SR: Default,
    {
        Self::with_runtime(f, SR::default())
    }

    pub fn with_runtime<F: Send + FnMut() -> T>(f: F, sr: SR) -> Self
    where
        T: Sized,
    {
        let this = Self(Signal::with_runtime(f, sr));
        this.0.pull();
        this
    }
}
