use std::{borrow::Borrow, ops::Deref, pin::Pin, sync::Mutex};

use isoprenoid::{
	raw::{NoCallbacks, RawSignal},
	runtime::SignalsRuntimeRef,
	slot::{Slot, Token},
};
use pin_project::pin_project;

use crate::traits::{Guard, Subscribable};

use super::Source;

#[pin_project]
#[must_use = "Signals do nothing unless they are polled or subscribed to."]
pub(crate) struct ComputedUncachedMut<T: Send, F: Send + FnMut() -> T, SR: SignalsRuntimeRef>(
	#[pin] RawSignal<ForceSyncUnpin<Mutex<F>>, (), SR>,
);

#[pin_project]
struct ForceSyncUnpin<T: ?Sized>(#[pin] T);
unsafe impl<T: ?Sized> Sync for ForceSyncUnpin<T> {}

pub(crate) struct ComputedUncachedMutGuard<T: ?Sized>(T);
pub(crate) struct ComputedUncachedMutGuardExclusive<T: ?Sized>(T);

impl<T: ?Sized> Guard<T> for ComputedUncachedMutGuard<T> {}
impl<T: ?Sized> Guard<T> for ComputedUncachedMutGuardExclusive<T> {}

impl<T: ?Sized> Deref for ComputedUncachedMutGuard<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T: ?Sized> Deref for ComputedUncachedMutGuardExclusive<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T: ?Sized> Borrow<T> for ComputedUncachedMutGuard<T> {
	fn borrow(&self) -> &T {
		self.0.borrow()
	}
}

impl<T: ?Sized> Borrow<T> for ComputedUncachedMutGuardExclusive<T> {
	fn borrow(&self) -> &T {
		self.0.borrow()
	}
}

/// TODO: Safety documentation.
unsafe impl<T: Send, F: Send + FnMut() -> T, SR: SignalsRuntimeRef + Sync> Sync
	for ComputedUncachedMut<T, F, SR>
{
}

impl<T: Send, F: Send + FnMut() -> T, SR: SignalsRuntimeRef> ComputedUncachedMut<T, F, SR> {
	pub(crate) fn new(fn_pin: F, runtime: SR) -> Self {
		Self(RawSignal::with_runtime(
			ForceSyncUnpin(fn_pin.into()),
			runtime,
		))
	}

	pub(crate) fn touch<'a>(self: Pin<&Self>) -> Pin<&Mutex<F>> {
		unsafe {
			self.project_ref()
				.0
				.project_or_init::<NoCallbacks>(|fn_pin, cache| Self::init(fn_pin, cache))
				.0
				.map_unchecked(|r| &r.0)
		}
	}
}

/// # Safety
///
/// These are the only functions that access `cache`.
/// Externally synchronised through guarantees on [`isoprenoid::raw::Callbacks`].
impl<T: Send, F: Send + FnMut() -> T, SR: SignalsRuntimeRef> ComputedUncachedMut<T, F, SR> {
	unsafe fn init<'a>(_: Pin<&'a ForceSyncUnpin<Mutex<F>>>, lazy: Slot<'a, ()>) -> Token<'a> {
		lazy.write(())
	}
}

impl<T: Send, F: Send + FnMut() -> T, SR: SignalsRuntimeRef> Source<T, SR>
	for ComputedUncachedMut<T, F, SR>
{
	fn touch(self: Pin<&Self>) {
		self.touch();
	}

	fn get_clone(self: Pin<&Self>) -> T
	where
		T: Sync + Clone,
	{
		self.get_clone_exclusive()
	}

	fn get_clone_exclusive(self: Pin<&Self>) -> T
	where
		T: Clone,
	{
		self.read_exclusive().0
	}

	fn read<'r>(self: Pin<&'r Self>) -> ComputedUncachedMutGuard<T>
	where
		Self: Sized,
		T: 'r + Sync,
	{
		ComputedUncachedMutGuard(self.read_exclusive().0)
	}

	type Read<'r> = ComputedUncachedMutGuard<T>
	where
		Self: 'r + Sized,
		T: 'r + Sync;

	fn read_exclusive<'r>(self: Pin<&'r Self>) -> ComputedUncachedMutGuardExclusive<T>
	where
		Self: Sized,
		T: 'r,
	{
		let mutex = self.touch();
		let mut fn_pin = mutex.lock().expect("unreachable");
		ComputedUncachedMutGuardExclusive(
			self.project_ref()
				.0
				.update_dependency_set(move |_, _| fn_pin()),
		)
	}

	type ReadExclusive<'r> = ComputedUncachedMutGuardExclusive<T>
	where
		Self: 'r + Sized,
		T: 'r;

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
}

impl<T: Send, F: Send + FnMut() -> T, SR: SignalsRuntimeRef> Subscribable<T, SR>
	for ComputedUncachedMut<T, F, SR>
{
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
