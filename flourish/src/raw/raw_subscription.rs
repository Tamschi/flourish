use std::{borrow::Borrow, pin::Pin};

use isoprenoid::runtime::SignalRuntimeRef;
use pin_project::pin_project;

use crate::traits::Subscribable;

use super::{RawCached, Source};

#[pin_project]
#[must_use = "Subscriptions are cancelled when dropped."]
#[repr(transparent)]
pub struct RawSubscription<
	//FIXME: Remove the `T: Clone` bound here, likely by using a different inner source,
	// without always caching. This would unlock **various** bounds relaxations! It may be
	// necessary to add a generic way to subscribe to sources, but it's possible that this
	// should be crate-private.
	T: Send + Clone,
	S: Subscribable<SR, Output = T>,
	SR: SignalRuntimeRef,
>(#[pin] RawCached<T, S, SR>);

//TODO: Add some associated methods, like not-boxing `read`/`read_exclusive`.
//TODO: Turn some of these functions into methods.

#[doc(hidden)]
pub fn new_raw_unsubscribed_subscription<
	T: Send + Clone,
	S: Subscribable<SR, Output = T>,
	SR: SignalRuntimeRef,
>(
	source: S,
) -> RawSubscription<T, S, SR> {
	RawSubscription(RawCached::new(source))
}

#[doc(hidden)]
pub fn pull_subscription<T: Send + Clone, S: Subscribable<SR, Output = T>, SR: SignalRuntimeRef>(
	subscription: Pin<&RawSubscription<T, S, SR>>,
) {
	subscription.project_ref().0.subscribe_inherently();
}

#[doc(hidden)]
pub fn pin_into_pin_impl_source<'a, T: Send + ?Sized, SR: SignalRuntimeRef>(
	pin: Pin<&'a impl Source<SR, Output = T>>,
) -> Pin<&'a impl Source<SR, Output = T>> {
	pin
}

impl<T: Send + Clone, S: Subscribable<SR, Output = T>, SR: SignalRuntimeRef> Source<SR>
	for RawSubscription<T, S, SR>
{
	type Output = T;

	fn touch(self: Pin<&Self>) {
		self.project_ref().0.touch();
	}

	fn get(self: Pin<&Self>) -> Self::Output
	where
		Self::Output: Sync + Copy,
	{
		self.project_ref().0.get()
	}

	fn get_clone(self: Pin<&Self>) -> Self::Output
	where
		Self::Output: Sync + Clone,
	{
		self.project_ref().0.get_clone()
	}

	fn get_exclusive(self: Pin<&Self>) -> Self::Output
	where
		Self::Output: Copy,
	{
		self.project_ref().0.get_exclusive()
	}

	fn get_clone_exclusive(self: Pin<&Self>) -> Self::Output
	where
		Self::Output: Clone,
	{
		self.project_ref().0.get_clone_exclusive()
	}

	fn read<'a>(self: Pin<&'a Self>) -> Box<dyn 'a + Borrow<Self::Output>>
	where
		Self::Output: Sync,
	{
		Box::new(self.project_ref().0.read())
	}

	fn read_exclusive<'a>(self: Pin<&'a Self>) -> Box<dyn 'a + Borrow<Self::Output>> {
		Box::new(self.project_ref().0.read_exclusive())
	}

	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		Source::clone_runtime_ref(&self.0)
	}
}
