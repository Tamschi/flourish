use std::{
	borrow::Borrow,
	ops::Deref,
	pin::Pin,
	sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use isoprenoid::{
	raw::{Callbacks, RawSignal},
	runtime::{CallbackTableTypes, Propagation, SignalsRuntimeRef},
	slot::{Slot, Token},
};
use pin_project::pin_project;

use crate::traits::{Guard, Subscribable};

use super::Source;

#[pin_project]
#[must_use = "Signals do nothing unless they are polled or subscribed to."]
pub(crate) struct Computed<T: Send, F: Send + FnMut() -> T, SR: SignalsRuntimeRef>(
	#[pin] RawSignal<ForceSyncUnpin<Mutex<F>>, ForceSyncUnpin<RwLock<T>>, SR>,
);

#[pin_project]
struct ForceSyncUnpin<T: ?Sized>(#[pin] T);
unsafe impl<T: ?Sized> Sync for ForceSyncUnpin<T> {}

pub(crate) struct ComputedGuard<'a, T: ?Sized>(RwLockReadGuard<'a, T>);
pub(crate) struct ComputedGuardExclusive<'a, T: ?Sized>(RwLockWriteGuard<'a, T>);

impl<'a, T: ?Sized> Guard<T> for ComputedGuard<'a, T> {}
impl<'a, T: ?Sized> Guard<T> for ComputedGuardExclusive<'a, T> {}

impl<'a, T: ?Sized> Deref for ComputedGuard<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		self.0.deref()
	}
}

impl<'a, T: ?Sized> Deref for ComputedGuardExclusive<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		self.0.deref()
	}
}

impl<'a, T: ?Sized> Borrow<T> for ComputedGuard<'a, T> {
	fn borrow(&self) -> &T {
		self.0.borrow()
	}
}

impl<'a, T: ?Sized> Borrow<T> for ComputedGuardExclusive<'a, T> {
	fn borrow(&self) -> &T {
		self.0.borrow()
	}
}

/// TODO: Safety documentation.
unsafe impl<T: Send, F: Send + FnMut() -> T, SR: SignalsRuntimeRef + Sync> Sync
	for Computed<T, F, SR>
{
}

impl<T: Send, F: Send + FnMut() -> T, SR: SignalsRuntimeRef> Computed<T, F, SR> {
	pub(crate) fn new(fn_pin: F, runtime: SR) -> Self {
		Self(RawSignal::with_runtime(
			ForceSyncUnpin(fn_pin.into()),
			runtime,
		))
	}

	pub(crate) fn touch(self: Pin<&Self>) -> Pin<&RwLock<T>> {
		unsafe {
			self.project_ref()
				.0
				.project_or_init::<E>(|fn_pin, cache| Self::init(fn_pin, cache))
				.1
				.project_ref()
				.0
		}
	}
}

enum E {}
impl<T: Send, F: Send + FnMut() -> T, SR: SignalsRuntimeRef>
	Callbacks<ForceSyncUnpin<Mutex<F>>, ForceSyncUnpin<RwLock<T>>, SR> for E
{
	const UPDATE: Option<
		fn(
			eager: Pin<&ForceSyncUnpin<Mutex<F>>>,
			lazy: Pin<&ForceSyncUnpin<RwLock<T>>>,
		) -> Propagation,
	> = {
		fn eval<T: Send, F: Send + FnMut() -> T>(
			fn_pin: Pin<&ForceSyncUnpin<Mutex<F>>>,
			cache: Pin<&ForceSyncUnpin<RwLock<T>>>,
		) -> Propagation {
			//FIXME: This is externally synchronised already.
			let new_value = fn_pin.project_ref().0.try_lock().expect("unreachable")();
			*cache.project_ref().0.write().unwrap() = new_value;
			Propagation::Propagate
		}
		Some(eval)
	};

	const ON_SUBSCRIBED_CHANGE: Option<
		fn(
			source: Pin<&RawSignal<ForceSyncUnpin<Mutex<F>>, ForceSyncUnpin<RwLock<T>>, SR>>,
			eager: Pin<&ForceSyncUnpin<Mutex<F>>>,
			lazy: Pin<&ForceSyncUnpin<RwLock<T>>>,
			subscribed: <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
		) -> Propagation,
	> = None;
}

/// # Safety
///
/// These are the only functions that access `cache`.
/// Externally synchronised through guarantees on [`isoprenoid::raw::Callbacks`].
impl<T: Send, F: Send + FnMut() -> T, SR: SignalsRuntimeRef> Computed<T, F, SR> {
	unsafe fn init<'a>(
		fn_pin: Pin<&'a ForceSyncUnpin<Mutex<F>>>,
		cache: Slot<'a, ForceSyncUnpin<RwLock<T>>>,
	) -> Token<'a> {
		cache.write(ForceSyncUnpin(
			//FIXME: This is technically already externally synchronised.
			fn_pin.project_ref().0.try_lock().expect("unreachable")().into(),
		))
	}
}

impl<T: Send, F: Send + FnMut() -> T, SR: SignalsRuntimeRef> Source<T, SR> for Computed<T, F, SR> {
	fn touch(self: Pin<&Self>) {
		self.touch();
	}

	fn get_clone(self: Pin<&Self>) -> T
	where
		T: Sync + Clone,
	{
		self.read().clone()
	}

	fn get_clone_exclusive(self: Pin<&Self>) -> T
	where
		T: Clone,
	{
		self.read_exclusive().clone()
	}

	fn read<'r>(self: Pin<&'r Self>) -> ComputedGuard<'r, T>
	where
		Self: Sized,
		T: 'r + Sync,
	{
		let touch = unsafe { Pin::into_inner_unchecked(self.touch()) };
		ComputedGuard(touch.read().unwrap())
	}

	type Read<'r> = ComputedGuard<'r, T>
	where
		Self: 'r + Sized,
		T: 'r + Sync;

	fn read_exclusive<'r>(self: Pin<&'r Self>) -> ComputedGuardExclusive<'r, T>
	where
		Self: Sized,
		T: 'r,
	{
		let touch = unsafe { Pin::into_inner_unchecked(self.touch()) };
		ComputedGuardExclusive(touch.write().unwrap())
	}

	type ReadExclusive<'r> = ComputedGuardExclusive<'r, T>
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
	for Computed<T, F, SR>
{
	fn subscribe(self: Pin<&Self>) {
		let signal = self.project_ref().0;
		signal.subscribe();
		signal.clone_runtime_ref().run_detached(|| {
			signal.project_or_init::<E>(|fn_pin, cache| unsafe { Self::init(fn_pin, cache) })
		});
	}

	fn unsubscribe(self: Pin<&Self>) {
		self.project_ref().0.unsubscribe()
	}
}
