use std::pin::Pin;

use isoprenoid::{
	raw::{NoCallbacks, RawSignal},
	runtime::SignalsRuntimeRef,
	slot::{Slot, Token},
};
use pin_project::pin_project;

use crate::traits::{Guard, UnmanagedSignal, ValueGuard};

#[pin_project]
#[must_use = "Signals do nothing unless they are polled or subscribed to."]
pub(crate) struct ComputedUncached<T: Send, F: Send + Sync + Fn() -> T, SR: SignalsRuntimeRef>(
	#[pin] RawSignal<ForceSyncUnpin<F>, (), SR>,
);

#[pin_project]
struct ForceSyncUnpin<T: ?Sized>(#[pin] T);
unsafe impl<T: ?Sized> Sync for ForceSyncUnpin<T> {}

// TODO: Safety documentation.
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

	pub(crate) fn touch<'a>(self: Pin<&Self>) -> Pin<&F> {
		unsafe {
			self.project_ref()
				.0
				.project_or_init::<NoCallbacks>(|fn_pin, cache| Self::init(fn_pin, cache))
				.0
				.map_unchecked(|r| &r.0)
		}
	}

	fn read_exclusive<'r>(self: Pin<&'r Self>) -> ValueGuard<T> {
		let fn_pin = self.touch();
		ValueGuard(
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
	unsafe fn init<'a>(_: Pin<&'a ForceSyncUnpin<F>>, lazy: Slot<'a, ()>) -> Token<'a> {
		lazy.write(())
	}
}

impl<T: Send, F: Send + Sync + Fn() -> T, SR: SignalsRuntimeRef> UnmanagedSignal<T, SR>
	for ComputedUncached<T, F, SR>
{
	fn touch(self: Pin<&Self>) {
		self.touch();
	}

	fn get_clone(self: Pin<&Self>) -> T
	where
		T: Sync + Clone,
	{
		self.read_exclusive().0
	}

	fn get_clone_exclusive(self: Pin<&Self>) -> T
	where
		T: Clone,
	{
		self.read_exclusive().0
	}

	fn read<'r>(self: Pin<&'r Self>) -> impl 'r + Guard<T>
	where
		Self: Sized,
		T: 'r + Sync,
	{
		self.read_exclusive()
	}

	fn read_exclusive<'r>(self: Pin<&'r Self>) -> impl 'r + Guard<T>
	where
		Self: Sized,
		T: 'r,
	{
		self.read_exclusive()
	}

	fn read_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r + Sync,
	{
		Box::new(self.read())
	}

	fn read_exclusive_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r,
	{
		Box::new(self.read_exclusive())
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
