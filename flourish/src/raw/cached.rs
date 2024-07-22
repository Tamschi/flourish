use std::{
	borrow::Borrow,
	mem,
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
pub(crate) struct Cached<T: Send + Clone, S: Subscribable<T, SR>, SR: SignalsRuntimeRef>(
	#[pin] RawSignal<ForceSyncUnpin<S>, ForceSyncUnpin<RwLock<T>>, SR>,
);

#[pin_project]
struct ForceSyncUnpin<T: ?Sized>(#[pin] T);
unsafe impl<T: ?Sized> Sync for ForceSyncUnpin<T> {}

pub(crate) struct CachedGuard<'a, T: ?Sized>(RwLockReadGuard<'a, T>);
pub(crate) struct CachedGuardExclusive<'a, T: ?Sized>(RwLockWriteGuard<'a, T>);

impl<'a, T: ?Sized> Deref for CachedGuard<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		self.borrow()
	}
}

impl<'a, T: ?Sized> Borrow<T> for CachedGuard<'a, T> {
	fn borrow(&self) -> &T {
		self.0.borrow()
	}
}

impl<'a, T: ?Sized> Deref for CachedGuardExclusive<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		self.borrow()
	}
}

impl<'a, T: ?Sized> Borrow<T> for CachedGuardExclusive<'a, T> {
	fn borrow(&self) -> &T {
		self.0.borrow()
	}
}

// TODO: Safety documentation.
unsafe impl<T: Send + Clone, S: Subscribable<T, SR>, SR: SignalsRuntimeRef + Sync> Sync
	for Cached<T, S, SR>
{
}

impl<T: Send + Clone, S: Subscribable<T, SR>, SR: SignalsRuntimeRef> Cached<T, S, SR> {
	pub(crate) fn new(source: S) -> Self {
		let runtime = source.clone_runtime_ref();
		Self(RawSignal::with_runtime(
			ForceSyncUnpin(source.into()),
			runtime,
		))
	}

	pub(crate) fn touch(self: Pin<&Self>) -> Pin<&RwLock<T>> {
		unsafe {
			self.project_ref()
				.0
				.project_or_init::<E>(|source, cache| Self::init(source, cache))
				.1
				.project_ref()
				.0
		}
	}

	pub(crate) fn subscribe_inherently(self: Pin<&Self>) -> Option<CachedGuard<T>> {
		Some(unsafe {
			//TODO: SAFETY COMMENT.
			mem::transmute::<CachedGuard<T>, CachedGuard<T>>(CachedGuard(
				self.project_ref()
					.0
					.subscribe_inherently_or_init::<E>(|source, cache| Self::init(source, cache))?
					.1
					.project_ref()
					.0
					.read()
					.unwrap(),
			))
		})
	}
}

enum E {}
impl<T: Send + Clone, S: Subscribable<T, SR>, SR: SignalsRuntimeRef>
	Callbacks<ForceSyncUnpin<S>, ForceSyncUnpin<RwLock<T>>, SR> for E
{
	const UPDATE: Option<
		fn(eager: Pin<&ForceSyncUnpin<S>>, lazy: Pin<&ForceSyncUnpin<RwLock<T>>>) -> Propagation,
	> = {
		fn eval<T: Send + Clone, S: Subscribable<T, SR>, SR: SignalsRuntimeRef>(
			source: Pin<&ForceSyncUnpin<S>>,
			cache: Pin<&ForceSyncUnpin<RwLock<T>>>,
		) -> Propagation {
			//FIXME: This can be split up to avoid congestion where not necessary.
			let new_value = source.project_ref().0.get_clone_exclusive();
			*cache.project_ref().0.write().unwrap() = new_value;
			Propagation::Propagate
		}
		Some(eval)
	};

	const ON_SUBSCRIBED_CHANGE: Option<
		fn(
			source: Pin<&RawSignal<ForceSyncUnpin<S>, ForceSyncUnpin<RwLock<T>>, SR>>,
			eager: Pin<&ForceSyncUnpin<S>>,
			lazy: Pin<&ForceSyncUnpin<RwLock<T>>>,
			subscribed: <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
		) -> Propagation,
	> = None;
}

/// # Safety
///
/// These are the only functions that access `cache`.
/// Externally synchronised through guarantees on [`isoprenoid::raw::Callbacks`].
impl<T: Send + Clone, S: Subscribable<T, SR>, SR: SignalsRuntimeRef> Cached<T, S, SR> {
	unsafe fn init<'a>(
		source: Pin<&'a ForceSyncUnpin<S>>,
		cache: Slot<'a, ForceSyncUnpin<RwLock<T>>>,
	) -> Token<'a> {
		cache.write(ForceSyncUnpin(
			//FIXME: This can be split up to avoid congestion where not necessary.
			source.project_ref().0.get_clone_exclusive().into(),
		))
	}
}

impl<T: Send + Clone, S: Subscribable<T, SR>, SR: SignalsRuntimeRef> Source<T, SR>
	for Cached<T, S, SR>
{
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
		self.touch().write().unwrap().clone()
	}

	fn read<'r>(self: Pin<&'r Self>) -> CachedGuard<'r, T>
	where
		Self: Sized,
		T: 'r + Sync,
	{
		let touch = unsafe { Pin::into_inner_unchecked(self.touch()) };
		CachedGuard(touch.read().unwrap())
	}

	type Read<'r> = CachedGuard<'r, T>
	where
		Self: 'r + Sized,
		T: Sync;

	fn read_exclusive<'r>(self: Pin<&'r Self>) -> CachedGuardExclusive<'r, T>
	where
		Self: Sized,
		T: 'r,
	{
		let touch = unsafe { Pin::into_inner_unchecked(self.touch()) };
		CachedGuardExclusive(touch.write().unwrap())
	}

	type ReadExclusive<'r> = CachedGuardExclusive<'r, T>
	where
		Self: 'r + Sized,
		T: 'r;

	fn read_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Borrow<T>>
	where
		T: 'r + Sync,
	{
		Box::new(self.read())
	}

	fn read_exclusive_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Borrow<T>>
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

impl<T: Send + Clone, S: Subscribable<T, SR>, SR: SignalsRuntimeRef> Subscribable<T, SR>
	for Cached<T, S, SR>
{
	fn subscribe_inherently<'r>(self: Pin<&'r Self>) -> Option<Box<dyn 'r + Borrow<T>>> {
		self.subscribe_inherently().map(|b| Box::new(b) as Box<_>)
	}

	fn unsubscribe_inherently(self: Pin<&Self>) -> bool {
		self.project_ref().0.unsubscribe_inherently()
	}
}
