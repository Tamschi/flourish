use std::{borrow::Borrow, pin::Pin};

use isoprenoid::{
	raw::{NoCallbacks, RawSignal},
	runtime::SignalsRuntimeRef,
	slot::{Slot, Written},
};
use pin_project::pin_project;

use crate::traits::Subscribable;

use super::Source;

#[pin_project]
#[must_use = "Signals do nothing unless they are polled or subscribed to."]
pub(crate) struct ComputedUncached<T: Send, F: Send + Sync + Fn() -> T, SR: SignalsRuntimeRef>(
	#[pin] RawSignal<ForceSyncUnpin<F>, (), SR>,
);

#[pin_project]
struct ForceSyncUnpin<T: ?Sized>(#[pin] T);
unsafe impl<T: ?Sized> Sync for ForceSyncUnpin<T> {}

/// TODO: Safety documentation.
unsafe impl<T: Send, F: Send + Sync + Fn() -> T, SR: SignalsRuntimeRef + Sync> Sync
	for ComputedUncached<T, F, SR>
{
}

impl<T: Send, F: Send + Sync + Fn() -> T, SR: SignalsRuntimeRef> ComputedUncached<T, F, SR> {
	pub(crate) fn new(fn_pin: F, runtime: SR) -> Self {
		Self(RawSignal::with_runtime(
			ForceSyncUnpin(fn_pin.into()),
			runtime,
		))
	}

	fn get(self: Pin<&Self>) -> T {
		let fn_pin = self.touch();
		self.project_ref()
			.0
			.update_dependency_set(move |_, _| fn_pin())
	}

	pub(crate) fn touch<'a>(self: Pin<&Self>) -> Pin<&F> {
		unsafe {
			self.project_ref()
				.0
				.project_or_init::<NoCallbacks>(|fn_pin, cache| Self::init(fn_pin, cache))
				.0
				.map_unchecked(|r| &r.0)
		}
	}

	fn subscribe_inherently<'a>(self: Pin<&'a Self>) -> Option<impl 'a + Borrow<T>> {
		let fn_pin = unsafe {
			self.project_ref()
				.0
				.subscribe_inherently_or_init::<NoCallbacks>(|fn_pin, cache| {
					Self::init(fn_pin, cache)
				})?
				.0
				.map_unchecked(|r| &r.0)
		};
		Some(
			self.project_ref()
				.0
				.update_dependency_set(move |_, _| fn_pin()),
		)
	}
}

/// # Safety
///
/// These are the only functions that access `cache`.
/// Externally synchronised through guarantees on [`isoprenoid::raw::Callbacks`].
impl<T: Send, F: Send + Sync + Fn() -> T, SR: SignalsRuntimeRef> ComputedUncached<T, F, SR> {
	unsafe fn init<'a>(_: Pin<&'a ForceSyncUnpin<F>>, lazy: Slot<'a, ()>) -> Written<'a, ()> {
		lazy.write(())
	}
}

impl<T: Send, F: Send + Sync + Fn() -> T, SR: SignalsRuntimeRef> Source<SR>
	for ComputedUncached<T, F, SR>
{
	type Output = T;

	fn touch(self: Pin<&Self>) {
		self.touch();
	}

	fn get(self: Pin<&Self>) -> Self::Output
	where
		Self::Output: Sync + Copy,
	{
		self.get()
	}

	fn get_clone(self: Pin<&Self>) -> Self::Output
	where
		Self::Output: Sync + Clone,
	{
		self.get()
	}

	fn get_exclusive(self: Pin<&Self>) -> Self::Output
	where
		Self::Output: Copy,
	{
		self.get()
	}

	fn get_clone_exclusive(self: Pin<&Self>) -> Self::Output
	where
		Self::Output: Clone,
	{
		self.get()
	}

	fn read<'a>(self: Pin<&'a Self>) -> Box<dyn 'a + Borrow<Self::Output>>
	where
		Self::Output: Sync,
	{
		Box::new(self.get())
	}

	fn read_exclusive<'a>(self: Pin<&'a Self>) -> Box<dyn 'a + Borrow<Self::Output>> {
		Box::new(self.get())
	}

	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		self.0.clone_runtime_ref()
	}
}

impl<T: Send, F: Send + Sync + Fn() -> T, SR: SignalsRuntimeRef> Subscribable<SR>
	for ComputedUncached<T, F, SR>
{
	fn subscribe_inherently<'r>(self: Pin<&'r Self>) -> Option<Box<dyn 'r + Borrow<Self::Output>>> {
		self.subscribe_inherently().map(|b| Box::new(b) as Box<_>)
	}

	fn unsubscribe_inherently(self: Pin<&Self>) -> bool {
		self.project_ref().0.unsubscribe_inherently()
	}
}
