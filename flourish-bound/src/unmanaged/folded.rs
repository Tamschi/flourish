use std::{
	borrow::Borrow,
	cell::{Ref, RefCell, UnsafeCell},
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
pub(crate) struct Folded<T, F: FnMut(&mut T) -> Propagation, SR: SignalsRuntimeRef>(
	#[pin] RawSignal<(RefCell<T>, UnsafeCell<F>), (), SR>,
);

pub(crate) struct FoldedGuard<'a, T: ?Sized>(Ref<'a, T>);

impl<'a, T: ?Sized> Guard<T> for FoldedGuard<'a, T> {}

impl<'a, T: ?Sized> Deref for FoldedGuard<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		self.0.deref()
	}
}

impl<'a, T: ?Sized> Borrow<T> for FoldedGuard<'a, T> {
	fn borrow(&self) -> &T {
		self.0.borrow()
	}
}

impl<T, F: FnMut(&mut T) -> Propagation, SR: SignalsRuntimeRef> Folded<T, F, SR> {
	pub(crate) fn new(init: T, fn_pin: F, runtime: SR) -> Self {
		Self(RawSignal::with_runtime(
			(init.into(), fn_pin.into()),
			runtime,
		))
	}

	pub(crate) fn touch(self: Pin<&Self>) -> &RefCell<T> {
		unsafe {
			&Pin::into_inner_unchecked(
				self.project_ref()
					.0
					.project_or_init::<E>(|state, cache| Self::init(state, cache))
					.0,
			)
			.0
		}
	}
}

enum E {}
impl<T, F: ?Sized + FnMut(&mut T) -> Propagation, SR: SignalsRuntimeRef>
	Callbacks<(RefCell<T>, UnsafeCell<F>), (), SR> for E
{
	const UPDATE: Option<
		fn(eager: Pin<&(RefCell<T>, UnsafeCell<F>)>, lazy: Pin<&()>) -> Propagation,
	> = {
		fn eval<T, F: ?Sized + FnMut(&mut T) -> Propagation>(
			state: Pin<&(RefCell<T>, UnsafeCell<F>)>,
			_: Pin<&()>,
		) -> Propagation {
			let fn_pin = unsafe {
				//SAFETY: This function has exclusive access to `state`.
				&mut *state.1.get()
			};
			fn_pin(&mut *state.0.borrow_mut())
		}
		Some(eval)
	};

	const ON_SUBSCRIBED_CHANGE: Option<
		fn(
			source: Pin<&RawSignal<(RefCell<T>, UnsafeCell<F>), (), SR>>,
			eager: Pin<&(RefCell<T>, UnsafeCell<F>)>,
			lazy: Pin<&()>,
			subscribed: <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
		) -> Propagation,
	> = None;
}

/// # Safety
///
/// These are the only functions that access `cache`.
/// Externally synchronised through guarantees on [`isoprenoid_bound::raw::Callbacks`].
impl<T, F: FnMut(&mut T) -> Propagation, SR: SignalsRuntimeRef> Folded<T, F, SR> {
	unsafe fn init<'a>(
		state: Pin<&'a (RefCell<T>, UnsafeCell<F>)>,
		lazy: Slot<'a, ()>,
	) -> Token<'a> {
		let mut guard = state.0.borrow_mut();
		let _ = (&mut *state.1.get())(&mut *guard);
		lazy.write(())
	}
}

impl<T, F: FnMut(&mut T) -> Propagation, SR: SignalsRuntimeRef> UnmanagedSignal<T, SR>
	for Folded<T, F, SR>
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

	fn read<'r>(self: Pin<&'r Self>) -> FoldedGuard<'r, T>
	where
		Self: Sized,
		T: 'r,
	{
		let touch = self.touch();
		FoldedGuard(touch.borrow())
	}

	type Read<'r>
		= FoldedGuard<'r, T>
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
			signal.project_or_init::<E>(|fn_pin, cache| unsafe { Self::init(fn_pin, cache) })
		});
	}

	fn unsubscribe(self: Pin<&Self>) {
		self.project_ref().0.unsubscribe()
	}
}
