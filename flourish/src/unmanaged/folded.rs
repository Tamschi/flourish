use std::{cell::UnsafeCell, pin::Pin, sync::RwLock};

use isoprenoid::{
	raw::{Callbacks, RawSignal},
	runtime::{CallbackTableTypes, Propagation, SignalsRuntimeRef},
	slot::{Slot, Token},
};
use pin_project::pin_project;

use crate::traits::{Guard, ReadGuard, UnmanagedSignal, WriteGuard};

#[pin_project]
#[must_use = "Signals do nothing unless they are polled or subscribed to."]
pub(crate) struct Folded<T: Send, F: Send + FnMut(&mut T) -> Propagation, SR: SignalsRuntimeRef>(
	#[pin] RawSignal<(ForceSyncUnpin<RwLock<T>>, ForceSyncUnpin<UnsafeCell<F>>), (), SR>,
);

#[pin_project]
struct ForceSyncUnpin<T: ?Sized>(T);
unsafe impl<T: ?Sized> Sync for ForceSyncUnpin<T> {}

// TODO: Safety documentation.
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

impl<T: Send, F: Send + FnMut(&mut T) -> Propagation, SR: SignalsRuntimeRef> UnmanagedSignal<T, SR>
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

	fn read<'r>(self: Pin<&'r Self>) -> impl 'r + Guard<T>
	where
		Self: Sized,
		T: 'r + Sync,
	{
		let touch = self.touch();
		ReadGuard(touch.read().unwrap())
	}

	fn read_exclusive<'r>(self: Pin<&'r Self>) -> impl 'r + Guard<T>
	where
		Self: Sized,
		T: 'r,
	{
		let touch = self.touch();
		WriteGuard(touch.write().unwrap())
	}

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
