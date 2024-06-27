use std::pin::Pin;

use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef};

use crate::{Computed, ComputedGuard, Source};

#[must_use = "Subscriptions are cancelled when dropped."]
pub struct Subscription<T: Send + ?Sized, SR: SignalRuntimeRef = GlobalSignalRuntime>(
    Computed<T, SR>,
);

//TODO: Implementations
pub struct SubscriptionGuard<'a, T>(ComputedGuard<'a, T>);

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
        let this = Self(Computed::with_runtime(f, runtime));
        this.0.pull();
        this
    }

    pub fn as_source(&self) -> Pin<&(dyn Source<Value = T> + Sync)>
    where
        SR: Sync,
    {
        self.0.as_source()
    }
}
