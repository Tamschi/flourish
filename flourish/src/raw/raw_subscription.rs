use std::{borrow::Borrow, pin::Pin};

use pin_project::pin_project;
use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef};

use crate::Source;

use super::RawCached;

#[pin_project]
#[must_use = "Subscriptions are cancelled when dropped."]
#[repr(transparent)]
pub struct RawSubscription<
    //FIXME: Remove the `T: Clone` bound here, likely be using a different inner source,
	// without always caching. This would unlock **various** bounds relaxations! It may be
	// necessary to add a generic way to subscribe to sources, but it's possible that this
	// should be crate-private.
    T: Send + Clone,
    S: Source<SR, Value = T>,
    SR: SignalRuntimeRef = GlobalSignalRuntime,
>(#[pin] RawCached<T, S, SR>);

//TODO: Add some associated methods, like not-boxing `read`/`read_exclusive`.
//TODO: Turn some of these functions into methods.

pub fn new_raw_unsubscribed_subscription<
    T: Send + Clone,
    S: Source<SR, Value = T>,
    SR: SignalRuntimeRef,
>(
    source: S,
) -> RawSubscription<T, S, SR> {
    RawSubscription(RawCached::new(source))
}

pub fn pull_subscription<T: Send + Clone, S: Source<SR, Value = T>, SR: SignalRuntimeRef>(
    subscription: Pin<&RawSubscription<T, S, SR>>,
) {
    subscription.project_ref().0.pull();
}

pub fn pin_into_pin_impl_source<'a, T: Send + ?Sized, SR: SignalRuntimeRef>(
    pin: Pin<&'a impl Source<SR, Value = T>>,
) -> Pin<&'a impl Source<SR, Value = T>> {
    pin
}

impl<T: Send + Clone, S: Source<SR, Value = T>, SR: SignalRuntimeRef> Source<SR>
    for RawSubscription<T, S, SR>
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

    fn read_exclusive<'a>(self: Pin<&'a Self>) -> Box<dyn 'a + Borrow<Self::Value>> {
        Box::new(self.project_ref().0.read_exclusive())
    }

    fn clone_runtime_ref(&self) -> SR
    where
        SR: Sized,
    {
        self.0.clone_runtime_ref()
    }
}
