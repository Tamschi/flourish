use std::pin::Pin;

use isoprenoid::runtime::SignalsRuntimeRef;
use pin_project::pin_project;

use crate::{traits::BorrowGuard, Guard};

use super::UnmanagedSignal;

#[pin_project]
#[derive(Debug)]
pub(crate) struct Shared<T: Send + Sync + ?Sized, SR: SignalsRuntimeRef> {
	runtime: SR,
	#[pin]
	value: T,
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

	fn read<'r>(self: Pin<&'r Self>) -> impl 'r + Guard<T>
	where
		Self: Sized,
		T: 'r + Sync,
	{
		BorrowGuard(&unsafe { Pin::into_inner_unchecked(self) }.value)
	}

	fn read_exclusive<'r>(self: Pin<&'r Self>) -> impl 'r + Guard<T>
	where
		Self: Sized,
		T: 'r,
	{
		BorrowGuard(&unsafe { Pin::into_inner_unchecked(self) }.value)
	}

	fn read_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + crate::Guard<T>>
	where
		T: 'r + Sync,
	{
		Box::new(BorrowGuard(
			&unsafe { Pin::into_inner_unchecked(self) }.value,
		))
	}

	fn read_exclusive_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + crate::Guard<T>>
	where
		T: 'r,
	{
		Box::new(BorrowGuard(
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
