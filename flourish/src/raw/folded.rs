use std::{
	borrow::{Borrow, BorrowMut},
	cell::UnsafeCell,
	mem,
	pin::Pin,
	sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use isoprenoid::{
	raw::{Callbacks, RawSignal},
	runtime::{CallbackTableTypes, SignalRuntimeRef, Update},
	slot::{Slot, Token},
};
use pin_project::pin_project;

use crate::traits::Subscribable;

use super::Source;

#[pin_project]
#[must_use = "Signals do nothing unless they are polled or subscribed to."]
pub(crate) struct Folded<T: Send, F: Send + FnMut(&mut T) -> Update, SR: SignalRuntimeRef>(
	#[pin] RawSignal<(ForceSyncUnpin<RwLock<T>>, ForceSyncUnpin<UnsafeCell<F>>), (), SR>,
);

#[pin_project]
struct ForceSyncUnpin<T: ?Sized>(T);
unsafe impl<T: ?Sized> Sync for ForceSyncUnpin<T> {}

struct FoldedGuard<'a, T: ?Sized>(RwLockReadGuard<'a, T>);
struct FoldedGuardExclusive<'a, T: ?Sized>(RwLockWriteGuard<'a, T>);

impl<'a, T: ?Sized> Borrow<T> for FoldedGuard<'a, T> {
	fn borrow(&self) -> &T {
		self.0.borrow()
	}
}

impl<'a, T: ?Sized> Borrow<T> for FoldedGuardExclusive<'a, T> {
	fn borrow(&self) -> &T {
		self.0.borrow()
	}
}

/// TODO: Safety documentation.
unsafe impl<T: Send, F: Send + FnMut(&mut T) -> Update, SR: SignalRuntimeRef + Sync> Sync
	for Folded<T, F, SR>
{
}

impl<T: Send, F: Send + FnMut(&mut T) -> Update, SR: SignalRuntimeRef> Folded<T, F, SR> {
	pub(crate) fn new(init: T, fn_pin: F, runtime: SR) -> Self {
		Self(RawSignal::with_runtime(
			(ForceSyncUnpin(init.into()), ForceSyncUnpin(fn_pin.into())),
			runtime,
		))
	}

	pub(crate) fn get(self: Pin<&Self>) -> T
	where
		T: Sync + Copy,
	{
		*self.read().borrow()
	}

	pub(crate) fn get_clone(self: Pin<&Self>) -> T
	where
		T: Sync + Clone,
	{
		self.read().borrow().clone()
	}

	pub(crate) fn read<'a>(self: Pin<&'a Self>) -> impl 'a + Borrow<T>
	where
		T: Sync,
	{
		FoldedGuard(self.touch().read().unwrap())
	}

	pub(crate) fn read_exclusive<'a>(self: Pin<&'a Self>) -> impl 'a + Borrow<T> {
		FoldedGuardExclusive(self.touch().write().unwrap())
	}

	pub(crate) fn get_exclusive(self: Pin<&Self>) -> T
	where
		T: Copy,
	{
		self.get_clone_exclusive()
	}

	pub(crate) fn get_clone_exclusive(self: Pin<&Self>) -> T
	where
		T: Clone,
	{
		self.touch().write().unwrap().clone()
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

	fn subscribe_inherently<'a>(self: Pin<&'a Self>) -> Option<impl 'a + Borrow<T>> {
		Some(unsafe {
			//TODO: SAFETY COMMENT.
			mem::transmute::<FoldedGuard<T>, FoldedGuard<T>>(FoldedGuard(
				self.project_ref()
					.0
					.subscribe_inherently_or_init::<E>(|fn_pin, cache| Self::init(fn_pin, cache))?
					.0
					 .0
					 .0
					.read()
					.unwrap(),
			))
		})
	}
}

enum E {}
impl<T: Send, F: Send + ?Sized + FnMut(&mut T) -> Update, SR: SignalRuntimeRef>
	Callbacks<(ForceSyncUnpin<RwLock<T>>, ForceSyncUnpin<UnsafeCell<F>>), (), SR> for E
{
	const UPDATE: Option<
		fn(
			eager: Pin<&(ForceSyncUnpin<RwLock<T>>, ForceSyncUnpin<UnsafeCell<F>>)>,
			lazy: Pin<&()>,
		) -> Update,
	> = {
		fn eval<T: Send, F: Send + ?Sized + FnMut(&mut T) -> Update>(
			state: Pin<&(ForceSyncUnpin<RwLock<T>>, ForceSyncUnpin<UnsafeCell<F>>)>,
			_: Pin<&()>,
		) -> Update {
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
		),
	> = None;
}

/// # Safety
///
/// These are the only functions that access `cache`.
/// Externally synchronised through guarantees on [`isoprenoid::raw::Callbacks`].
impl<T: Send, F: Send + FnMut(&mut T) -> Update, SR: SignalRuntimeRef> Folded<T, F, SR> {
	unsafe fn init<'a>(
		state: Pin<&'a (ForceSyncUnpin<RwLock<T>>, ForceSyncUnpin<UnsafeCell<F>>)>,
		lazy: Slot<'a, ()>,
	) -> Token<'a> {
		let mut guard = state.0 .0.try_write().expect("unreachable");
		let _ = (&mut *state.1 .0.get())(guard.borrow_mut());
		lazy.write(())
	}
}

impl<T: Send, F: Send + FnMut(&mut T) -> Update, SR: SignalRuntimeRef> Source<SR>
	for Folded<T, F, SR>
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
		self.get_clone()
	}

	fn get_exclusive(self: Pin<&Self>) -> Self::Output
	where
		Self::Output: Copy,
	{
		self.get_exclusive()
	}

	fn get_clone_exclusive(self: Pin<&Self>) -> Self::Output
	where
		Self::Output: Clone,
	{
		self.get_clone_exclusive()
	}

	fn read<'a>(self: Pin<&'a Self>) -> Box<dyn 'a + Borrow<Self::Output>>
	where
		Self::Output: Sync,
	{
		Box::new(self.read())
	}

	fn read_exclusive<'a>(self: Pin<&'a Self>) -> Box<dyn 'a + Borrow<Self::Output>> {
		Box::new(self.read_exclusive())
	}

	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		self.0.clone_runtime_ref()
	}
}

impl<T: Send, F: Send + FnMut(&mut T) -> Update, SR: SignalRuntimeRef> Subscribable<SR>
	for Folded<T, F, SR>
{
	fn subscribe_inherently<'r>(self: Pin<&'r Self>) -> Option<Box<dyn 'r + Borrow<Self::Output>>> {
		self.subscribe_inherently().map(|b| Box::new(b) as Box<_>)
	}

	fn unsubscribe_inherently(self: Pin<&Self>) -> bool {
		self.project_ref().0.unsubscribe_inherently()
	}
}
