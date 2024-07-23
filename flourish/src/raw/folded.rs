use std::{
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

use crate::traits::Subscribable;

use super::Source;

#[pin_project]
#[must_use = "Signals do nothing unless they are polled or subscribed to."]
pub(crate) struct Folded<T: Send, F: Send + FnMut(&mut T) -> Propagation, SR: SignalsRuntimeRef>(
	#[pin] RawSignal<(ForceSyncUnpin<RwLock<T>>, ForceSyncUnpin<UnsafeCell<F>>), (), SR>,
);

#[pin_project]
struct ForceSyncUnpin<T: ?Sized>(T);
unsafe impl<T: ?Sized> Sync for ForceSyncUnpin<T> {}

struct FoldedGuard<'a, T: ?Sized>(RwLockReadGuard<'a, T>);
struct FoldedGuardExclusive<'a, T: ?Sized>(RwLockWriteGuard<'a, T>);

impl<'a, T: ?Sized> Deref for FoldedGuard<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		self.0.deref()
	}
}

impl<'a, T: ?Sized> Deref for FoldedGuardExclusive<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		self.0.deref()
	}
}

/// TODO: Safety documentation.
unsafe impl<T: Send, F: Send + FnMut(&mut T) -> Propagation, SR: SignalsRuntimeRef + Sync> Sync
	for Folded<T, F, SR>
{
}

impl<T: Send, F: Send + FnMut(&mut T) -> Propagation, SR: SignalsRuntimeRef> Folded<T, F, SR> {
	pub(crate) fn new(init: T, fn_pin: F, runtime: SR) -> Self {
		Self(RawSignal::with_runtime(
			(ForceSyncUnpin(init.into()), ForceSyncUnpin(fn_pin.into())),
			runtime,
		))
	}

	pub(crate) fn touch(self: Pin<&Self>) -> &RwLock<T> {
		unsafe {
			&Pin::into_inner_unchecked(
				self.project_ref()
					.0
					.project_or_init::<E>(|state, cache| Self::init(state, cache))
					.0,
			)
			.0
			 .0
		}
	}
}

enum E {}
impl<T: Send, F: Send + ?Sized + FnMut(&mut T) -> Propagation, SR: SignalsRuntimeRef>
	Callbacks<(ForceSyncUnpin<RwLock<T>>, ForceSyncUnpin<UnsafeCell<F>>), (), SR> for E
{
	const UPDATE: Option<
		fn(
			eager: Pin<&(ForceSyncUnpin<RwLock<T>>, ForceSyncUnpin<UnsafeCell<F>>)>,
			lazy: Pin<&()>,
		) -> Propagation,
	> = {
		fn eval<T: Send, F: Send + ?Sized + FnMut(&mut T) -> Propagation>(
			state: Pin<&(ForceSyncUnpin<RwLock<T>>, ForceSyncUnpin<UnsafeCell<F>>)>,
			_: Pin<&()>,
		) -> Propagation {
			let fn_pin = unsafe {
				//SAFETY: This function has exclusive access to `state`.
				&mut *state.1 .0.get()
			};
			fn_pin(&mut *state.0 .0.write().unwrap())
		}
		Some(eval)
	};

	const ON_SUBSCRIBED_CHANGE: Option<
		fn(
			source: Pin<
				&RawSignal<(ForceSyncUnpin<RwLock<T>>, ForceSyncUnpin<UnsafeCell<F>>), (), SR>,
			>,
			eager: Pin<&(ForceSyncUnpin<RwLock<T>>, ForceSyncUnpin<UnsafeCell<F>>)>,
			lazy: Pin<&()>,
			subscribed: <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
		) -> Propagation,
	> = None;
}

/// # Safety
///
/// These are the only functions that access `cache`.
/// Externally synchronised through guarantees on [`isoprenoid::raw::Callbacks`].
impl<T: Send, F: Send + FnMut(&mut T) -> Propagation, SR: SignalsRuntimeRef> Folded<T, F, SR> {
	unsafe fn init<'a>(
		state: Pin<&'a (ForceSyncUnpin<RwLock<T>>, ForceSyncUnpin<UnsafeCell<F>>)>,
		lazy: Slot<'a, ()>,
	) -> Token<'a> {
		let mut guard = state.0 .0.try_write().expect("unreachable");
		let _ = (&mut *state.1 .0.get())(&mut *guard);
		lazy.write(())
	}
}

impl<T: Send, F: Send + FnMut(&mut T) -> Propagation, SR: SignalsRuntimeRef> Source<T, SR>
	for Folded<T, F, SR>
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

	fn read<'r>(self: Pin<&'r Self>) -> FoldedGuard<'r, T>
	where
		Self: Sized,
		T: 'r + Sync,
	{
		let touch = self.touch();
		FoldedGuard(touch.read().unwrap())
	}

	type Read<'r> = FoldedGuard<'r, T>
	where
		Self: 'r + Sized,
		T: 'r + Sync;

	fn read_exclusive<'r>(self: Pin<&'r Self>) -> FoldedGuardExclusive<'r, T>
	where
		Self: Sized,
		T: 'r,
	{
		let touch = self.touch();
		FoldedGuardExclusive(touch.write().unwrap())
	}

	type ReadExclusive<'r> = FoldedGuardExclusive<'r, T>
	where
		Self: 'r + Sized,
		T: 'r;

	fn read_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Deref<Target = T>>
	where
		T: 'r + Sync,
	{
		Box::new(self.read())
	}

	fn read_exclusive_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Deref<Target = T>>
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

impl<T: Send, F: Send + FnMut(&mut T) -> Propagation, SR: SignalsRuntimeRef> Subscribable<T, SR>
	for Folded<T, F, SR>
{
	fn subscribe_inherently(self: Pin<&Self>) -> bool {
		self.project_ref()
			.0
			.subscribe_inherently_or_init::<E>(|fn_pin, cache| unsafe { Self::init(fn_pin, cache) })
			.is_some()
	}

	fn unsubscribe_inherently(self: Pin<&Self>) -> bool {
		self.project_ref().0.unsubscribe_inherently()
	}
}
