use std::{borrow::Borrow, ops::Deref, pin::Pin};

use isoprenoid_unsend::runtime::SignalsRuntimeRef;
use pin_project::pin_project;

use crate::Guard;

use super::UnmanagedSignal;

#[pin_project]
#[derive(Debug)]
pub(crate) struct Shared<T: ?Sized, SR: SignalsRuntimeRef> {
	runtime: SR,
	#[pin]
	value: T,
}

pub(crate) struct SharedGuard<'a, T: ?Sized>(&'a T);

impl<T: ?Sized> Guard<T> for SharedGuard<'_, T> {}

impl<T: ?Sized> Deref for SharedGuard<'_, T> {
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

impl<T: ?Sized, SR: SignalsRuntimeRef> Shared<T, SR> {
	pub(crate) fn with_runtime(value: T, runtime: SR) -> Self
	where
		T: Sized,
	{
		Self { value, runtime }
	}
}

impl<T: ?Sized, SR: SignalsRuntimeRef> UnmanagedSignal<T, SR> for Shared<T, SR> {
	fn touch(self: Pin<&Self>) {
		// No effect.
	}

	fn get_clone(self: Pin<&Self>) -> T
	where
		T: Clone,
	{
		self.value.clone()
	}

	fn read<'r>(self: Pin<&'r Self>) -> Self::Read<'r>
	where
		Self: Sized,
		T: 'r,
	{
		SharedGuard(&unsafe { Pin::into_inner_unchecked(self) }.value)
	}

	type Read<'r>
		= SharedGuard<'r, T>
	where
		Self: 'r + Sized,
		T: 'r;

	fn read_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + crate::Guard<T>>
	where
		T: 'r,
	{
		Box::new(SharedGuard(
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
