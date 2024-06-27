use std::{borrow::Borrow, pin::Pin};

use pin_project::pin_project;
use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef};

use crate::Source;

use super::{RawComputed, RawComputedGuard};

#[pin_project]
#[must_use = "Subscriptions are cancelled when dropped."]
#[repr(transparent)]
pub struct RawSubscription<
    T: Send,
    F: Send + ?Sized + FnMut() -> T,
    SR: SignalRuntimeRef = GlobalSignalRuntime,
>(#[pin] RawComputed<T, F, SR>);

//TODO: Implementations
pub struct RawSubscriptionGuard<'a, T>(RawComputedGuard<'a, T>);

/// See [rust-lang#98931](https://github.com/rust-lang/rust/issues/98931).
impl<T: Send, F: Send + ?Sized + FnMut() -> T> RawSubscription<T, F> {
    //TODO
}

impl<T: Send, F: Send + ?Sized + FnMut() -> T, SR: SignalRuntimeRef> RawSubscription<T, F, SR> {
    //TODO
}

pub fn new_raw_unsubscribed_subscription_with_runtime<
    T: Send,
    F: Send + FnMut() -> T,
    SR: SignalRuntimeRef,
>(
    f: F,
    runtime: SR,
) -> RawSubscription<T, F, SR> {
    RawSubscription(RawComputed::with_runtime(f, runtime))
}

pub fn pull_subscription<T: Send, F: Send + FnMut() -> T, SR: SignalRuntimeRef>(
    subscription: Pin<&RawSubscription<T, F, SR>>,
) {
    subscription.project_ref().0.pull();
}

pub fn pin_into_pin_impl_source<'a, T: Send + ?Sized, SR: SignalRuntimeRef>(
    pin: Pin<&'a impl Source<SR, Value = T>>,
) -> Pin<&'a impl Source<SR, Value = T>> {
    pin
}

impl<T: Send, F: Send + FnMut() -> T, SR: SignalRuntimeRef> Source<SR>
    for RawSubscription<T, F, SR>
{
    type Value = T;

    fn touch(self: Pin<&Self>) {
        self.project_ref().0.touch();
    }

    fn get(self: Pin<&Self>) -> Self::Value
    where
        Self::Value: Sync + Copy,
    {
        self.project_ref().0.get()
    }

    fn get_clone(self: Pin<&Self>) -> Self::Value
    where
        Self::Value: Sync + Clone,
    {
        self.project_ref().0.get_clone()
    }

    fn get_exclusive(self: Pin<&Self>) -> Self::Value
    where
        Self::Value: Copy,
    {
        self.project_ref().0.get_exclusive()
    }

    fn get_clone_exclusive(self: Pin<&Self>) -> Self::Value
    where
        Self::Value: Clone,
    {
        self.project_ref().0.get_clone_exclusive()
    }

    fn read<'a>(self: Pin<&'a Self>) -> Box<dyn 'a + Borrow<Self::Value>>
    where
        Self::Value: 'a + Sync,
    {
        Box::new(self.project_ref().0.read())
    }

    fn clone_runtime_ref(&self) -> SR
    where
        SR: Sized,
    {
        self.0.clone_runtime_ref()
    }
}
