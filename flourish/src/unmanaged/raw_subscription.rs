use std::pin::Pin;

use isoprenoid::runtime::SignalsRuntimeRef;
use pin_project::pin_project;

use crate::traits::{Guard, UnmanagedSignal};

use super::Cached;

#[pin_project]
#[must_use = "Subscriptions are cancelled when dropped."]
#[repr(transparent)]
pub struct RawSubscription<
	//FIXME: Remove the `T: Clone` bound here, likely by using a different inner source,
	// without always caching. This would unlock **various** bounds relaxations! It may be
	// necessary to add a generic way to subscribe to sources, but it's possible that this
	// should be crate-private.
	T: Send + Clone,
	S: UnmanagedSignal<T, SR>,
	SR: SignalsRuntimeRef,
>(#[pin] Cached<T, S, SR>);

//TODO: Turn some of these functions into methods.

#[doc(hidden)]
pub fn new_raw_unsubscribed_subscription<
	T: Send + Clone,
	S: UnmanagedSignal<T, SR>,
	SR: SignalsRuntimeRef,
>(
	source: S,
) -> RawSubscription<T, S, SR> {
	RawSubscription(Cached::new(source))
}

#[doc(hidden)]
pub fn pull_new_subscription<T: Send + Clone, S: UnmanagedSignal<T, SR>, SR: SignalsRuntimeRef>(
	subscription: Pin<&RawSubscription<T, S, SR>>,
) {
	subscription.project_ref().0.subscribe()
}

#[doc(hidden)]
pub fn pin_into_pin_impl_source<'a, T: Send + ?Sized, SR: SignalsRuntimeRef>(
	pin: Pin<&'a impl UnmanagedSignal<T, SR>>,
) -> Pin<&'a impl UnmanagedSignal<T, SR>> {
	pin
}

/// Note that `subscribe` and `unsubscribe` have no effect on [`RawSubscription`]!
impl<T: Send + Clone, S: UnmanagedSignal<T, SR>, SR: SignalsRuntimeRef> UnmanagedSignal<T, SR>
	for RawSubscription<T, S, SR>
{
	fn touch(self: Pin<&Self>) {
		self.project_ref().0.touch();
	}

	fn get(self: Pin<&Self>) -> T
	where
		T: Sync + Copy,
	{
		self.project_ref().0.get()
	}

	fn get_clone(self: Pin<&Self>) -> T
	where
		T: Sync + Clone,
	{
		self.project_ref().0.get_clone()
	}

	fn get_exclusive(self: Pin<&Self>) -> T
	where
		T: Copy,
	{
		self.project_ref().0.get_exclusive()
	}

	fn get_clone_exclusive(self: Pin<&Self>) -> T
	where
		T: Clone,
	{
		self.project_ref().0.get_clone_exclusive()
	}

	fn read<'r>(self: Pin<&'r Self>) -> impl 'r + Guard<T>
	where
		Self: Sized,
		T: 'r + Sync,
	{
		self.project_ref().0.read()
	}

	fn read_exclusive<'r>(self: Pin<&'r Self>) -> impl 'r + Guard<T>
	where
		Self: Sized,
		T: 'r,
	{
		self.project_ref().0.read_exclusive()
	}

	fn read_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r + Sync,
	{
		self.project_ref().0.read_dyn()
	}

	fn read_exclusive_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r,
	{
		self.project_ref().0.read_exclusive_dyn()
	}

	fn subscribe(self: Pin<&Self>) {}

	fn unsubscribe(self: Pin<&Self>) {}

	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		UnmanagedSignal::clone_runtime_ref(&self.0)
	}
}
