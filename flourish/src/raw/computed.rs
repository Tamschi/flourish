use std::{
	borrow::Borrow,
	mem,
	pin::Pin,
	sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use isoprenoid::{
	raw::{Callbacks, RawSignal},
	runtime::{CallbackTableTypes, Propagation, SignalsRuntimeRef},
	slot::{Slot, Token},
};
use pin_project::pin_project;

use crate::traits::Subscribable;

use super::Source;

#[pin_project]
#[must_use = "Signals do nothing unless they are polled or subscribed to."]
pub(crate) struct Computed<T: Send, F: Send + FnMut() -> T, SR: SignalsRuntimeRef>(
	#[pin] RawSignal<ForceSyncUnpin<Mutex<F>>, ForceSyncUnpin<RwLock<T>>, SR>,
);

#[pin_project]
struct ForceSyncUnpin<T: ?Sized>(#[pin] T);
unsafe impl<T: ?Sized> Sync for ForceSyncUnpin<T> {}

struct ComputedGuard<'a, T: ?Sized>(RwLockReadGuard<'a, T>);
struct ComputedGuardExclusive<'a, T: ?Sized>(RwLockWriteGuard<'a, T>);

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
		Borrow::<T>::borrow(&self.read()).clone()
	}

	fn get_clone_exclusive(self: Pin<&Self>) -> T
	where
		T: Clone,
	{
		Borrow::<T>::borrow(&self.read_exclusive()).clone()
	}

	fn read<'r>(self: Pin<&'r Self>) -> ComputedGuard<'r, T>
	where
		Self: Sized,
		T: Sync,
	{
		let touch = unsafe { Pin::into_inner_unchecked(self.touch()) };
		ComputedGuard(touch.read().unwrap())
	}

	type Read<'r> = ComputedGuard<'r, T>
			where
				Self: 'r + Sized,
				T: Sync;

	fn read_exclusive<'r>(self: Pin<&'r Self>) -> ComputedGuardExclusive<'r, T>
	where
		Self: Sized,
	{
		let touch = unsafe { Pin::into_inner_unchecked(self.touch()) };
		ComputedGuardExclusive(touch.write().unwrap())
	}

	type ReadExclusive<'r> = ComputedGuardExclusive<'r, T>
			where
				Self: 'r + Sized;

	fn read_dyn<'a>(self: Pin<&'a Self>) -> Box<dyn 'a + Borrow<T>>
	where
		T: Sync,
	{
		Box::new(self.read())
	}

	fn read_exclusive_dyn<'a>(self: Pin<&'a Self>) -> Box<dyn 'a + Borrow<T>> {
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
	fn subscribe_inherently<'r>(self: Pin<&'r Self>) -> Option<Box<dyn 'r + Borrow<T>>> {
		Some(Box::new(unsafe {
			//TODO: SAFETY COMMENT.
			mem::transmute::<ComputedGuard<T>, ComputedGuard<T>>(ComputedGuard(
				self.project_ref()
					.0
					.subscribe_inherently_or_init::<E>(|fn_pin, cache| Self::init(fn_pin, cache))?
					.1
					.project_ref()
					.0
					.read()
					.unwrap(),
			))
		}))
	}

	fn unsubscribe_inherently(self: Pin<&Self>) -> bool {
		self.project_ref().0.unsubscribe_inherently()
	}
}
