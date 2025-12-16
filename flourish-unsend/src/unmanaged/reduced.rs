use std::{
	borrow::Borrow,
	cell::{Ref, RefCell, UnsafeCell},
	ops::Deref,
	pin::Pin,
};

use isoprenoid_unsend::{
	raw::{Callbacks, RawSignal},
	runtime::{CallbackTableTypes, Propagation, SignalsRuntimeRef},
	slot::{Slot, Token},
};
use pin_project::pin_project;

use crate::traits::{Guard, UnmanagedSignal};

#[pin_project]
#[must_use = "Signals do nothing unless they are polled or subscribed to."]
pub(crate) struct Reduced<
	T,
	S: FnMut() -> T,
	M: FnMut(&mut T, T) -> Propagation,
	SR: SignalsRuntimeRef,
>(#[pin] RawSignal<UnsafeCell<(S, M)>, RefCell<T>, SR>);

pub(crate) struct ReducedGuard<'a, T: ?Sized>(Ref<'a, T>);

impl<'a, T: ?Sized> Guard<T> for ReducedGuard<'a, T> {}

impl<'a, T: ?Sized> Deref for ReducedGuard<'a, T> {
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

impl<T, S: FnMut() -> T, M: FnMut(&mut T, T) -> Propagation, SR: SignalsRuntimeRef>
	Reduced<T, S, M, SR>
{
	pub(crate) fn new(select_fn_pin: S, reduce_fn_pin: M, runtime: SR) -> Self {
		Self(RawSignal::with_runtime(
			(select_fn_pin, reduce_fn_pin).into(),
			runtime,
		))
	}

	pub(crate) fn touch(self: Pin<&Self>) -> &RefCell<T> {
		unsafe {
			self.project_ref()
				.0
				.project_or_init::<E>(|state, cache| Self::init(state, cache))
				.1
				.get_ref()
		}
	}
}

enum E {}
impl<T, S: FnMut() -> T, M: ?Sized + FnMut(&mut T, T) -> Propagation, SR: SignalsRuntimeRef>
	Callbacks<UnsafeCell<(S, M)>, RefCell<T>, SR> for E
{
	const UPDATE: Option<
		fn(eager: Pin<&UnsafeCell<(S, M)>>, lazy: Pin<&RefCell<T>>) -> Propagation,
	> = {
		fn eval<T, S: FnMut() -> T, M: ?Sized + FnMut(&mut T, T) -> Propagation>(
			state: Pin<&UnsafeCell<(S, M)>>,
			cache: Pin<&RefCell<T>>,
		) -> Propagation {
			let (select_fn_pin, reduce_fn_pin) = unsafe {
				//SAFETY: This function has exclusive access to `state`.
				&mut *state.get()
			};
			// TODO: Split this up to avoid congestion where possible.
			let next_value = select_fn_pin();
			reduce_fn_pin(&mut *cache.borrow_mut(), next_value)
		}
		Some(eval)
	};

	const ON_SUBSCRIBED_CHANGE: Option<
		fn(
			source: Pin<&RawSignal<UnsafeCell<(S, M)>, RefCell<T>, SR>>,
			eager: Pin<&UnsafeCell<(S, M)>>,
			lazy: Pin<&RefCell<T>>,
			subscribed: <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
		) -> Propagation,
	> = None;
}

/// # Safety
///
/// These are the only functions that access `cache`.
/// Externally synchronised through guarantees on [`isoprenoid_unsend::raw::Callbacks`].
impl<T, S: FnMut() -> T, M: FnMut(&mut T, T) -> Propagation, SR: SignalsRuntimeRef>
	Reduced<T, S, M, SR>
{
	unsafe fn init<'a>(
		state: Pin<&'a UnsafeCell<(S, M)>>,
		cache: Slot<'a, RefCell<T>>,
	) -> Token<'a> {
		cache.write((&mut *state.get()).0().into())
	}
}

impl<T, S: FnMut() -> T, M: FnMut(&mut T, T) -> Propagation, SR: SignalsRuntimeRef>
	UnmanagedSignal<T, SR> for Reduced<T, S, M, SR>
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

	fn read<'r>(self: Pin<&'r Self>) -> ReducedGuard<'r, T>
	where
		Self: Sized,
		T: 'r,
	{
		let touch = self.touch();
		ReducedGuard(touch.borrow())
	}

	type Read<'r>
		= ReducedGuard<'r, T>
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
			signal.project_or_init::<E>(|f, cache| unsafe { Self::init(f, cache) })
		});
	}

	fn unsubscribe(self: Pin<&Self>) {
		self.project_ref().0.unsubscribe()
	}
}
