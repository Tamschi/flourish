use std::{
	borrow::Borrow,
	cell::{Ref, RefCell},
	ops::Deref,
	pin::Pin,
};

use isoprenoid_bound::{
	raw::{Callbacks, RawSignal},
	runtime::{CallbackTableTypes, Propagation, SignalsRuntimeRef},
	slot::{Slot, Token},
};
use pin_project::pin_project;

use crate::traits::{Guard, UnmanagedSignal};

#[pin_project]
#[must_use = "Signals do nothing unless they are polled or subscribed to."]
pub(crate) struct Cached<T: Clone, S: UnmanagedSignal<T, SR>, SR: SignalsRuntimeRef>(
	#[pin] RawSignal<S, RefCell<T>, SR>,
);

pub(crate) struct CachedGuard<'a, T: ?Sized>(Ref<'a, T>);

impl<'a, T: ?Sized> Guard<T> for CachedGuard<'a, T> {}

impl<'a, T: ?Sized> Deref for CachedGuard<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		self.0.deref()
	}
}

impl<'a, T: ?Sized> Borrow<T> for CachedGuard<'a, T> {
	fn borrow(&self) -> &T {
		self.0.borrow()
	}
}

impl<T: Clone, S: UnmanagedSignal<T, SR>, SR: SignalsRuntimeRef> Cached<T, S, SR> {
	pub(crate) fn new(source: S) -> Self {
		let runtime = source.clone_runtime_ref();
		Self(RawSignal::with_runtime(source.into(), runtime))
	}

	pub(crate) fn touch(self: Pin<&Self>) -> Pin<&RefCell<T>> {
		unsafe {
			self.project_ref()
				.0
				.project_or_init::<E>(|source, cache| Self::init(source, cache))
				.1
		}
	}
}

enum E {}
impl<T: Clone, S: UnmanagedSignal<T, SR>, SR: SignalsRuntimeRef> Callbacks<S, RefCell<T>, SR>
	for E
{
	const UPDATE: Option<fn(eager: Pin<&S>, lazy: Pin<&RefCell<T>>) -> Propagation> = {
		fn eval<T: Clone, S: UnmanagedSignal<T, SR>, SR: SignalsRuntimeRef>(
			source: Pin<&S>,
			cache: Pin<&RefCell<T>>,
		) -> Propagation {
			//FIXME: This can be split up to avoid congestion where not necessary.
			let new_value = source.get_clone();
			*cache.borrow_mut() = new_value;
			Propagation::Propagate
		}
		Some(eval)
	};

	const ON_SUBSCRIBED_CHANGE: Option<
		fn(
			source: Pin<&RawSignal<S, RefCell<T>, SR>>,
			eager: Pin<&S>,
			lazy: Pin<&RefCell<T>>,
			subscribed: <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
		) -> Propagation,
	> = None;
}

/// # Safety
///
/// These are the only functions that access `cache`.
/// Externally synchronised through guarantees on [`isoprenoid_bound::raw::Callbacks`].
impl<T: Clone, S: UnmanagedSignal<T, SR>, SR: SignalsRuntimeRef> Cached<T, S, SR> {
	unsafe fn init<'a>(source: Pin<&'a S>, cache: Slot<'a, RefCell<T>>) -> Token<'a> {
		cache.write(source.get_clone().into())
	}
}

impl<T: Clone, S: UnmanagedSignal<T, SR>, SR: SignalsRuntimeRef> UnmanagedSignal<T, SR>
	for Cached<T, S, SR>
{
	fn touch(self: Pin<&Self>) {
		self.touch();
	}

	fn get_clone(self: Pin<&Self>) -> T
	where
		T: Clone,
	{
		self.read().clone()
	}

	fn read<'r>(self: Pin<&'r Self>) -> CachedGuard<'r, T>
	where
		Self: Sized,
		T: 'r,
	{
		let touch = unsafe { Pin::into_inner_unchecked(self.touch()) };
		CachedGuard(touch.borrow())
	}

	type Read<'r>
		= CachedGuard<'r, T>
	where
		Self: 'r + Sized;

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
			signal.project_or_init::<E>(|source, cache| unsafe { Self::init(source, cache) })
		});
	}

	fn unsubscribe(self: Pin<&Self>) {
		self.project_ref().0.unsubscribe()
	}
}
