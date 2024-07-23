use std::{borrow::Borrow, ops::Deref, pin::Pin};

use isoprenoid::runtime::SignalsRuntimeRef;
use pin_project::pin_project;

use crate::traits::{Guard, Subscribable};

use super::{
	cached::{CachedGuard, CachedGuardExclusive},
	Cached, Source,
};

#[pin_project]
#[must_use = "Subscriptions are cancelled when dropped."]
#[repr(transparent)]
pub struct RawSubscription<
	//FIXME: Remove the `T: Clone` bound here, likely by using a different inner source,
	// without always caching. This would unlock **various** bounds relaxations! It may be
	// necessary to add a generic way to subscribe to sources, but it's possible that this
	// should be crate-private.
	T: Send + Clone,
	S: Subscribable<T, SR>,
	SR: SignalsRuntimeRef,
>(#[pin] Cached<T, S, SR>);

pub struct RawSubscriptionGuard<'a, T: ?Sized>(CachedGuard<'a, T>);
pub struct RawSubscriptionGuardExclusive<'a, T: ?Sized>(CachedGuardExclusive<'a, T>);

impl<'a, T: ?Sized> Guard<T> for RawSubscriptionGuard<'a, T> {}
impl<'a, T: ?Sized> Guard<T> for RawSubscriptionGuardExclusive<'a, T> {}

impl<'a, T: ?Sized> Deref for RawSubscriptionGuard<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		self.0.deref()
	}
}

impl<'a, T: ?Sized> Deref for RawSubscriptionGuardExclusive<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		self.0.deref()
	}
}

impl<'a, T: ?Sized> Borrow<T> for RawSubscriptionGuard<'a, T> {
	fn borrow(&self) -> &T {
		self.0.borrow()
	}
}

impl<'a, T: ?Sized> Borrow<T> for RawSubscriptionGuardExclusive<'a, T> {
	fn borrow(&self) -> &T {
		self.0.borrow()
	}
}

//TODO: Turn some of these functions into methods.

#[doc(hidden)]
pub fn new_raw_unsubscribed_subscription<
	T: Send + Clone,
	S: Subscribable<T, SR>,
	SR: SignalsRuntimeRef,
>(
	source: S,
) -> RawSubscription<T, S, SR> {
	RawSubscription(Cached::new(source))
}

#[doc(hidden)]
pub fn pull_new_subscription<T: Send + Clone, S: Subscribable<T, SR>, SR: SignalsRuntimeRef>(
	subscription: Pin<&RawSubscription<T, S, SR>>,
) {
	assert!(subscription.project_ref().0.subscribe_inherently());
}

#[doc(hidden)]
pub fn pin_into_pin_impl_source<'a, T: Send + ?Sized, SR: SignalsRuntimeRef>(
	pin: Pin<&'a impl Source<T, SR>>,
) -> Pin<&'a impl Source<T, SR>> {
	pin
}

impl<T: Send + Clone, S: Subscribable<T, SR>, SR: SignalsRuntimeRef> Source<T, SR>
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

	fn read<'r>(self: Pin<&'r Self>) -> RawSubscriptionGuard<'r, T>
	where
		Self: Sized,
		T: 'r + Sync,
	{
		RawSubscriptionGuard(self.project_ref().0.read())
	}

	type Read<'r> = RawSubscriptionGuard<'r, T>
	where
		Self: 'r + Sized,
		T: 'r + Sync;

	fn read_exclusive<'r>(self: Pin<&'r Self>) -> RawSubscriptionGuardExclusive<'r, T>
	where
		Self: Sized,
		T: 'r,
	{
		RawSubscriptionGuardExclusive(self.project_ref().0.read_exclusive())
	}

	type ReadExclusive<'r> = RawSubscriptionGuardExclusive<'r, T>
	where
		Self: 'r + Sized,
		T: 'r;

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

	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		Source::clone_runtime_ref(&self.0)
	}
}
