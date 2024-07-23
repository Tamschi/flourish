use std::{
	borrow::Borrow,
	cell::UnsafeCell,
	ops::Deref,
	pin::Pin,
	sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
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
pub(crate) struct Reduced<
	T: Send,
	S: Send + FnMut() -> T,
	M: Send + FnMut(&mut T, T) -> Propagation,
	SR: SignalsRuntimeRef,
>(#[pin] RawSignal<ForceSyncUnpin<UnsafeCell<(S, M)>>, ForceSyncUnpin<RwLock<T>>, SR>);

#[pin_project]
struct ForceSyncUnpin<T: ?Sized>(T);
unsafe impl<T: ?Sized> Sync for ForceSyncUnpin<T> {}

pub(crate) struct ReducedGuard<'a, T: ?Sized>(RwLockReadGuard<'a, T>);
pub(crate) struct ReducedGuardExclusive<'a, T: ?Sized>(RwLockWriteGuard<'a, T>);

impl<'a, T: ?Sized> Guard<T> for ReducedGuard<'a, T> {}
impl<'a, T: ?Sized> Guard<T> for ReducedGuardExclusive<'a, T> {}

impl<'a, T: ?Sized> Deref for ReducedGuard<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		self.0.deref()
	}
}

impl<'a, T: ?Sized> Deref for ReducedGuardExclusive<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		self.0.deref()
	}
}

impl<'a, T: ?Sized> Borrow<T> for ReducedGuard<'a, T> {
	fn borrow(&self) -> &T {
		self.0.borrow()
	}
}

impl<'a, T: ?Sized> Borrow<T> for ReducedGuardExclusive<'a, T> {
	fn borrow(&self) -> &T {
		self.0.borrow()
	}
}

/// TODO: Safety documentation.
unsafe impl<
		T: Send,
		S: Send + FnMut() -> T,
		M: Send + FnMut(&mut T, T) -> Propagation,
		SR: SignalsRuntimeRef + Sync,
	> Sync for Reduced<T, S, M, SR>
{
}

impl<
		T: Send,
		S: Send + FnMut() -> T,
		M: Send + FnMut(&mut T, T) -> Propagation,
		SR: SignalsRuntimeRef,
	> Reduced<T, S, M, SR>
{
	pub(crate) fn new(select_fn_pin: S, reduce_fn_pin: M, runtime: SR) -> Self {
		Self(RawSignal::with_runtime(
			ForceSyncUnpin((select_fn_pin, reduce_fn_pin).into()),
			runtime,
		))
	}

	pub(crate) fn touch(self: Pin<&Self>) -> &RwLock<T> {
		unsafe {
			self.project_ref()
				.0
				.project_or_init::<E>(|state, cache| Self::init(state, cache))
				.1
				.project_ref()
				.0
		}
	}
}

enum E {}
impl<
		T: Send,
		S: Send + FnMut() -> T,
		M: Send + ?Sized + FnMut(&mut T, T) -> Propagation,
		SR: SignalsRuntimeRef,
	> Callbacks<ForceSyncUnpin<UnsafeCell<(S, M)>>, ForceSyncUnpin<RwLock<T>>, SR> for E
{
	const UPDATE: Option<
		fn(
			eager: Pin<&ForceSyncUnpin<UnsafeCell<(S, M)>>>,
			lazy: Pin<&ForceSyncUnpin<RwLock<T>>>,
		) -> Propagation,
	> = {
		fn eval<
			T: Send,
			S: Send + FnMut() -> T,
			M: Send + ?Sized + FnMut(&mut T, T) -> Propagation,
		>(
			state: Pin<&ForceSyncUnpin<UnsafeCell<(S, M)>>>,
			cache: Pin<&ForceSyncUnpin<RwLock<T>>>,
		) -> Propagation {
			let (select_fn_pin, reduce_fn_pin) = unsafe {
				//SAFETY: This function has exclusive access to `state`.
				&mut *state.0.get()
			};
			// TODO: Split this up to avoid congestion where possible.
			let next_value = select_fn_pin();
			reduce_fn_pin(&mut *cache.project_ref().0.write().unwrap(), next_value)
		}
		Some(eval)
	};

	const ON_SUBSCRIBED_CHANGE: Option<
		fn(
			source: Pin<
				&RawSignal<ForceSyncUnpin<UnsafeCell<(S, M)>>, ForceSyncUnpin<RwLock<T>>, SR>,
			>,
			eager: Pin<&ForceSyncUnpin<UnsafeCell<(S, M)>>>,
			lazy: Pin<&ForceSyncUnpin<RwLock<T>>>,
			subscribed: <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
		) -> Propagation,
	> = None;
}

/// # Safety
///
/// These are the only functions that access `cache`.
/// Externally synchronised through guarantees on [`isoprenoid::raw::Callbacks`].
impl<
		T: Send,
		S: Send + FnMut() -> T,
		M: Send + FnMut(&mut T, T) -> Propagation,
		SR: SignalsRuntimeRef,
	> Reduced<T, S, M, SR>
{
	unsafe fn init<'a>(
		state: Pin<&'a ForceSyncUnpin<UnsafeCell<(S, M)>>>,
		cache: Slot<'a, ForceSyncUnpin<RwLock<T>>>,
	) -> Token<'a> {
		cache.write(ForceSyncUnpin((&mut *state.0.get()).0().into()))
	}
}

impl<
		T: Send,
		S: Send + FnMut() -> T,
		M: Send + FnMut(&mut T, T) -> Propagation,
		SR: SignalsRuntimeRef,
	> Source<T, SR> for Reduced<T, S, M, SR>
{
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

	fn read<'r>(self: Pin<&'r Self>) -> ReducedGuard<'r, T>
	where
		Self: Sized,
		T: 'r + Sync,
	{
		let touch = self.touch();
		ReducedGuard(touch.read().unwrap())
	}

	type Read<'r> = ReducedGuard<'r, T>
	where
		Self: 'r + Sized,
		T: 'r + Sync;

	fn read_exclusive<'r>(self: Pin<&'r Self>) -> ReducedGuardExclusive<'r, T>
	where
		Self: Sized,
		T: 'r,
	{
		let touch = self.touch();
		ReducedGuardExclusive(touch.write().unwrap())
	}

	type ReadExclusive<'r> = ReducedGuardExclusive<'r, T>
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

impl<
		T: Send,
		S: Send + FnMut() -> T,
		M: Send + FnMut(&mut T, T) -> Propagation,
		SR: SignalsRuntimeRef,
	> Subscribable<T, SR> for Reduced<T, S, M, SR>
{
	fn subscribe_inherently(self: Pin<&Self>) -> bool {
		self.project_ref()
			.0
			.subscribe_inherently_or_init::<E>(|f, cache| unsafe { Self::init(f, cache) })
			.is_some()
	}

	fn unsubscribe_inherently(self: Pin<&Self>) -> bool {
		self.project_ref().0.unsubscribe_inherently()
	}
}
