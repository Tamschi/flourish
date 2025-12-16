use std::{
	borrow::Borrow,
	cell::{Ref, RefCell},
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
pub(crate) struct Computed<T, F: FnMut() -> T, SR: SignalsRuntimeRef>(
	#[pin] RawSignal<RefCell<F>, RefCell<T>, SR>,
);

pub(crate) struct ComputedGuard<'a, T: ?Sized>(Ref<'a, T>);

impl<'a, T: ?Sized> Guard<T> for ComputedGuard<'a, T> {}

impl<'a, T: ?Sized> Deref for ComputedGuard<'a, T> {
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

impl<T, F: FnMut() -> T, SR: SignalsRuntimeRef> Computed<T, F, SR> {
	pub(crate) fn new(fn_pin: F, runtime: SR) -> Self {
		Self(RawSignal::with_runtime(fn_pin.into(), runtime))
	}

	pub(crate) fn touch(self: Pin<&Self>) -> Pin<&RefCell<T>> {
		unsafe {
			self.project_ref()
				.0
				.project_or_init::<E>(|fn_pin, cache| Self::init(fn_pin, cache))
				.1
		}
	}
}

enum E {}
impl<T, F: FnMut() -> T, SR: SignalsRuntimeRef> Callbacks<RefCell<F>, RefCell<T>, SR> for E {
	const UPDATE: Option<fn(eager: Pin<&RefCell<F>>, lazy: Pin<&RefCell<T>>) -> Propagation> = {
		fn eval<T, F: FnMut() -> T>(
			fn_pin: Pin<&RefCell<F>>,
			cache: Pin<&RefCell<T>>,
		) -> Propagation {
			//FIXME: This is externally synchronised already.
			let new_value = fn_pin.borrow_mut()();
			*cache.borrow_mut() = new_value;
			Propagation::Propagate
		}
		Some(eval)
	};

	const ON_SUBSCRIBED_CHANGE: Option<
		fn(
			source: Pin<&RawSignal<RefCell<F>, RefCell<T>, SR>>,
			eager: Pin<&RefCell<F>>,
			lazy: Pin<&RefCell<T>>,
			subscribed: <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
		) -> Propagation,
	> = None;
}

/// # Safety
///
/// These are the only functions that access `cache`.
/// Externally synchronised through guarantees on [`isoprenoid_unsend::raw::Callbacks`].
impl<T, F: FnMut() -> T, SR: SignalsRuntimeRef> Computed<T, F, SR> {
	unsafe fn init<'a>(fn_pin: Pin<&'a RefCell<F>>, cache: Slot<'a, RefCell<T>>) -> Token<'a> {
		cache.write(
			//FIXME: This is technically already externally synchronised.
			fn_pin.borrow_mut()().into(),
		)
	}
}

impl<T, F: FnMut() -> T, SR: SignalsRuntimeRef> UnmanagedSignal<T, SR> for Computed<T, F, SR> {
	fn touch(self: Pin<&Self>) {
		self.touch();
	}

	fn get_clone(self: Pin<&Self>) -> T
	where
		T: Clone,
	{
		self.read().clone()
	}

	fn read<'r>(self: Pin<&'r Self>) -> ComputedGuard<'r, T>
	where
		Self: Sized,
		T: 'r,
	{
		let touch = unsafe { Pin::into_inner_unchecked(self.touch()) };
		ComputedGuard(touch.borrow())
	}

	type Read<'r>
		= ComputedGuard<'r, T>
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
