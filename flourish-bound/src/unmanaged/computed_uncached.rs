use std::{borrow::Borrow, ops::Deref, pin::Pin};

use isoprenoid_bound::{
	raw::{NoCallbacks, RawSignal},
	runtime::SignalsRuntimeRef,
	slot::{Slot, Token},
};
use pin_project::pin_project;

use crate::traits::{Guard, UnmanagedSignal};

#[pin_project]
#[must_use = "Signals do nothing unless they are polled or subscribed to."]
pub(crate) struct ComputedUncached<T, F: Fn() -> T, SR: SignalsRuntimeRef>(
	#[pin] RawSignal<F, (), SR>,
);

pub(crate) struct ComputedUncachedGuard<T: ?Sized>(T);

impl<T: ?Sized> Guard<T> for ComputedUncachedGuard<T> {}

impl<T: ?Sized> Deref for ComputedUncachedGuard<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T: ?Sized> Borrow<T> for ComputedUncachedGuard<T> {
	fn borrow(&self) -> &T {
		self.0.borrow()
	}
}

impl<T, F: Fn() -> T, SR: SignalsRuntimeRef> ComputedUncached<T, F, SR> {
	pub(crate) fn new(fn_pin: F, runtime: SR) -> Self {
		Self(RawSignal::with_runtime(fn_pin, runtime))
	}

	pub(crate) fn touch<'a>(self: Pin<&Self>) -> Pin<&F> {
		unsafe {
			self.project_ref()
				.0
				.project_or_init::<NoCallbacks>(|fn_pin, cache| Self::init(fn_pin, cache))
				.0
		}
	}
}

/// # Safety
///
/// These are the only functions that access `cache`.
/// Externally synchronised through guarantees on [`isoprenoid_bound::raw::Callbacks`].
impl<T, F: Fn() -> T, SR: SignalsRuntimeRef> ComputedUncached<T, F, SR> {
	unsafe fn init<'a>(_: Pin<&'a F>, lazy: Slot<'a, ()>) -> Token<'a> {
		lazy.write(())
	}
}

impl<T, F: Fn() -> T, SR: SignalsRuntimeRef> UnmanagedSignal<T, SR> for ComputedUncached<T, F, SR> {
	fn touch(self: Pin<&Self>) {
		self.touch();
	}

	fn get_clone(self: Pin<&Self>) -> T
	where
		T: Clone,
	{
		self.read().0
	}

	fn read<'r>(self: Pin<&'r Self>) -> ComputedUncachedGuard<T>
	where
		Self: Sized,
		T: 'r,
	{
		let fn_pin = self.touch();
		ComputedUncachedGuard(
			self.project_ref()
				.0
				.update_dependency_set(move |_, _| fn_pin()),
		)
	}

	type Read<'r>
		= ComputedUncachedGuard<T>
	where
		Self: 'r + Sized,
		T: 'r;

	fn read_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r,
	{
		Box::new(self.read())
	}

	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		self.0.clone_runtime_ref()
	}

	fn subscribe(self: Pin<&Self>) {
		let signal = self.project_ref().0;
		signal.subscribe();
		signal.clone_runtime_ref().run_detached(|| {
			signal.project_or_init::<NoCallbacks>(|fn_pin, cache| unsafe {
				Self::init(fn_pin, cache)
			})
		});
	}

	fn unsubscribe(self: Pin<&Self>) {
		self.project_ref().0.unsubscribe()
	}
}
