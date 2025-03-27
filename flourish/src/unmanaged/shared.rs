use std::{borrow::Borrow, ops::Deref, pin::Pin};

use isoprenoid::runtime::SignalsRuntimeRef;
use pin_project::pin_project;

use crate::Guard;

use super::UnmanagedSignal;

#[pin_project]
#[derive(Debug)]
pub(crate) struct Shared<T: Send + Sync + ?Sized, SR: SignalsRuntimeRef> {
	runtime: SR,
	#[pin]
	value: T,
}

pub(crate) struct SharedGuard<'a, T: ?Sized>(&'a T);
pub(crate) struct SharedGuardExclusive<'a, T: ?Sized>(&'a T);

impl<T: ?Sized> Guard<T> for SharedGuard<'_, T> {}
impl<T: ?Sized> Guard<T> for SharedGuardExclusive<'_, T> {}

impl<T: ?Sized> Deref for SharedGuard<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		self.0
	}
}

impl<T: ?Sized> Deref for SharedGuardExclusive<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		self.0
	}
}

impl<T: ?Sized> Borrow<T> for SharedGuard<'_, T> {
	fn borrow(&self) -> &T {
		self.0
	}
}

impl<T: ?Sized> Borrow<T> for SharedGuardExclusive<'_, T> {
	fn borrow(&self) -> &T {
		self.0
	}
}

impl<T: Send + Sync + ?Sized, SR: SignalsRuntimeRef> Shared<T, SR> {
	pub(crate) fn with_runtime(value: T, runtime: SR) -> Self
	where
		T: Sized,
	{
		Self { value, runtime }
	}
}

impl<T: Send + Sync + ?Sized, SR: SignalsRuntimeRef> UnmanagedSignal<T, SR> for Shared<T, SR> {
	fn touch(self: Pin<&Self>) {
		// No effect.
	}

	fn get_clone(self: Pin<&Self>) -> T
	where
		T: Sync + Clone,
	{
		self.value.clone()
	}

	fn get_clone_exclusive(self: Pin<&Self>) -> T
	where
		T: Clone,
	{
		self.value.clone()
	}

	fn read<'r>(self: Pin<&'r Self>) -> Self::Read<'r>
	where
		Self: Sized,
		T: 'r + Sync,
	{
		SharedGuard(&unsafe { Pin::into_inner_unchecked(self) }.value)
	}

	type Read<'r>
		= SharedGuard<'r, T>
	where
		Self: 'r + Sized,
		T: 'r + Sync;

	fn read_exclusive<'r>(self: Pin<&'r Self>) -> Self::ReadExclusive<'r>
	where
		Self: Sized,
		T: 'r,
	{
		SharedGuardExclusive(&unsafe { Pin::into_inner_unchecked(self) }.value)
	}

	type ReadExclusive<'r>
		= SharedGuardExclusive<'r, T>
	where
		Self: 'r + Sized,
		T: 'r;

	fn read_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + crate::Guard<T>>
	where
		T: 'r + Sync,
	{
		Box::new(SharedGuard(
			&unsafe { Pin::into_inner_unchecked(self) }.value,
		))
	}

	fn read_exclusive_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + crate::Guard<T>>
	where
		T: 'r,
	{
		Box::new(SharedGuardExclusive(
			&unsafe { Pin::into_inner_unchecked(self) }.value,
		))
	}

	fn subscribe(self: Pin<&Self>) {
		// No effect.
	}

	fn unsubscribe(self: Pin<&Self>) {
		// No effect.
	}

	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		self.runtime.clone()
	}
}

impl<T: Send + Sync, SR: SignalsRuntimeRef> From<T> for Shared<T, SR>
where
	SR: Default,
{
	fn from(value: T) -> Self {
		Self::with_runtime(value, SR::default())
	}
}
