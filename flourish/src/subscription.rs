use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef};

use crate::{source::DelegateSource, Signal, SignalGuard};

#[must_use = "Subscriptions are cancelled when dropped."]
pub struct Subscription<T: Send + ?Sized, SR: SignalRuntimeRef = GlobalSignalRuntime>(
    Signal<T, SR>,
);

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
    pub fn with_runtime<F: Send + FnMut() -> T>(f: F, sr: SR) -> Self
    where
        T: Sized,
    {
        let this = Self(Signal::with_runtime(f, sr));
        this.0.pull();
        this
    }
}

impl<T: Send + ?Sized, SR: SignalRuntimeRef> DelegateSource for Subscription<T, SR> {
    type DelegateValue = T;

    fn delegate_source(&self) -> &impl crate::Source<Value = Self::DelegateValue> {
        &self.0
    }
}
