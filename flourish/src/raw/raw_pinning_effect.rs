use std::{borrow::BorrowMut, pin::Pin, sync::Mutex};

use isoprenoid::{
	raw::{Callbacks, RawSignal},
	runtime::{CallbackTableTypes, Propagation, SignalRuntimeRef},
	slot::{Slot, Written},
};
use pin_project::pin_project;

#[must_use = "Effects are cancelled when dropped."]
#[repr(transparent)]
pub struct RawPinningEffect<
	T: Send,
	S: Send + FnMut() -> (T, A),
	A: Send + FnOnce(Pin<&mut T>),
	D: Send + FnMut(Pin<&mut T>),
	SR: SignalRuntimeRef,
>(RawSignal<ForceSyncUnpin<Mutex<(S, D)>>, ForceSyncUnpin<Mutex<Option<T>>>, SR>);

#[pin_project]
struct ForceSyncUnpin<T: ?Sized>(#[pin] T);
unsafe impl<T: ?Sized> Sync for ForceSyncUnpin<T> {}

//TODO: Add some associated methods, like not-boxing `read`/`read_exclusive`.
//TODO: Turn some of these functions into methods.

#[doc(hidden)]
pub fn new_raw_unsubscribed_pinning_effect<
	T: Send,
	S: Send + FnMut() -> (T, A),
	A: Send + FnOnce(Pin<&mut T>),
	D: Send + FnMut(Pin<&mut T>),
	SR: SignalRuntimeRef,
>(
	fn_pin: S,
	before_drop_fn_pin: D,
	runtime: SR,
) -> RawPinningEffect<T, S, A, D, SR> {
	RawPinningEffect(RawSignal::with_runtime(
		ForceSyncUnpin((fn_pin, before_drop_fn_pin).into()),
		runtime,
	))
}

impl<
		T: Send,
		S: Send + FnMut() -> (T, A),
		A: Send + FnOnce(Pin<&mut T>),
		D: Send + FnMut(Pin<&mut T>),
		SR: SignalRuntimeRef,
	> Drop for RawPinningEffect<T, S, A, D, SR>
{
	fn drop(&mut self) {
		unsafe { Pin::new_unchecked(&mut self.0) }.deinit_and(|eager, lazy| {
			let before_drop = &mut eager.0.try_lock().unwrap().1;
			lazy.0
				.try_lock()
				.unwrap()
				.borrow_mut()
				.as_mut()
				.map(|value| unsafe { before_drop(Pin::new_unchecked(value)) });
		});
	}
}

enum E {}
impl<
		T: Send,
		S: Send + FnMut() -> (T, A),
		A: Send + FnOnce(Pin<&mut T>),
		D: Send + FnMut(Pin<&mut T>),
		SR: SignalRuntimeRef,
	> Callbacks<ForceSyncUnpin<Mutex<(S, D)>>, ForceSyncUnpin<Mutex<Option<T>>>, SR> for E
{
	const UPDATE: Option<
		fn(
			eager: Pin<&ForceSyncUnpin<Mutex<(S, D)>>>,
			lazy: Pin<&ForceSyncUnpin<Mutex<Option<T>>>>,
		) -> isoprenoid::runtime::Propagation,
	> = {
		fn eval<
			T: Send,
			S: Send + FnMut() -> (T, A),
			A: Send + FnOnce(Pin<&mut T>),
			D: Send + FnMut(Pin<&mut T>),
		>(
			callbacks: Pin<&ForceSyncUnpin<Mutex<(S, D)>>>,
			cache: Pin<&ForceSyncUnpin<Mutex<Option<T>>>>,
		) -> Propagation {
			let (init, before_drop) = &mut *callbacks.0.lock().expect("unreachable");
			let cache = &mut *cache.0.lock().expect("unreachable");
			cache
				.as_mut()
				.map(|value| unsafe { before_drop(Pin::new_unchecked(value)) });
			let (t, a) = init();
			*cache = Some(t);
			unsafe { a(Pin::new_unchecked(cache.as_mut().expect("unreachable"))) }
			Propagation::Halt
		}
		Some(eval)
	};

	const ON_SUBSCRIBED_CHANGE: Option<
		fn(
			signal: Pin<
				&RawSignal<ForceSyncUnpin<Mutex<(S, D)>>, ForceSyncUnpin<Mutex<Option<T>>>, SR>,
			>,
			eager: Pin<&ForceSyncUnpin<Mutex<(S, D)>>>,
			lazy: Pin<&ForceSyncUnpin<Mutex<Option<T>>>>,
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
		S: Send + FnMut() -> (T, A),
		A: Send + FnOnce(Pin<&mut T>),
		D: Send + FnMut(Pin<&mut T>),
		SR: SignalRuntimeRef,
	> RawPinningEffect<T, S, A, D, SR>
{
	unsafe fn init<'a>(
		callbacks: Pin<&'a ForceSyncUnpin<Mutex<(S, D)>>>,
		cache: Slot<'a, ForceSyncUnpin<Mutex<Option<T>>>>,
	) -> Written<'a, ForceSyncUnpin<Mutex<Option<T>>>> {
		let (t, a) = callbacks.project_ref().0.lock().expect("unreachable").0();
		let token = cache.write(ForceSyncUnpin(Some(t).into()));
		unsafe {
			a(Pin::new_unchecked(
				&mut *token.0.lock().unwrap().as_mut().expect("unreachable"),
			))
		};
		token
	}

	pub fn pull(self: Pin<&RawPinningEffect<T, S, A, D, SR>>) {
		self.0.clone_runtime_ref().run_detached(|| unsafe {
			Pin::new_unchecked(&self.0).subscribe_inherently_or_init::<E>(|callbacks, cache| {
				RawPinningEffect::<T, S, A, D, SR>::init(callbacks, cache)
			});
		})
	}
}
