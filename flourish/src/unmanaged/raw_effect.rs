use std::{ops::DerefMut as _, pin::Pin, sync::Mutex};

use isoprenoid::{
	raw::{Callbacks, RawSignal},
	runtime::{CallbackTableTypes, Propagation, SignalsRuntimeRef},
	slot::{Slot, Token},
};
use pin_project::pin_project;

#[must_use = "Effects are cancelled when dropped."]
#[repr(transparent)]
pub struct RawEffect<T: Send, S: Send + FnMut() -> T, D: Send + FnMut(T), SR: SignalsRuntimeRef>(
	RawSignal<ForceSyncUnpin<Mutex<(S, D)>>, ForceSyncUnpin<Mutex<Option<T>>>, SR>,
);

#[pin_project]
struct ForceSyncUnpin<T: ?Sized>(#[pin] T);
unsafe impl<T: ?Sized> Sync for ForceSyncUnpin<T> {}

//TODO: Add some associated methods, like not-boxing `read`/`read_exclusive`.
//TODO: Turn some of these functions into methods.

#[doc(hidden)]
pub fn new_raw_unsubscribed_effect<
	T: Send,
	S: Send + FnMut() -> T,
	D: Send + FnMut(T),
	SR: SignalsRuntimeRef,
>(
	init_fn_pin: S,
	drop_fn_pin: D,
	runtime: SR,
) -> RawEffect<T, S, D, SR> {
	RawEffect(RawSignal::with_runtime(
		ForceSyncUnpin((init_fn_pin, drop_fn_pin).into()),
		runtime,
	))
}

impl<T: Send, S: Send + FnMut() -> T, D: Send + FnMut(T), SR: SignalsRuntimeRef> Drop
	for RawEffect<T, S, D, SR>
{
	fn drop(&mut self) {
		let raw_signal = unsafe { Pin::new_unchecked(&mut self.0) };
		raw_signal.purge_and_deinit_with(|eager, lazy| {
			let drop = &mut eager.0.try_lock().unwrap().1;
			lazy.0
				.try_lock()
				.unwrap()
				.deref_mut()
				.take()
				.and_then(|value| Some(drop(value)));
		});
	}
}

enum E {}
impl<T: Send, S: Send + FnMut() -> T, D: Send + FnMut(T), SR: SignalsRuntimeRef>
	Callbacks<ForceSyncUnpin<Mutex<(S, D)>>, ForceSyncUnpin<Mutex<Option<T>>>, SR> for E
{
	const UPDATE: Option<
		fn(
			eager: Pin<&ForceSyncUnpin<Mutex<(S, D)>>>,
			lazy: Pin<&ForceSyncUnpin<Mutex<Option<T>>>>,
		) -> isoprenoid::runtime::Propagation,
	> = {
		fn eval<T: Send, S: Send + FnMut() -> T, D: Send + FnMut(T)>(
			source: Pin<&ForceSyncUnpin<Mutex<(S, D)>>>,
			cache: Pin<&ForceSyncUnpin<Mutex<Option<T>>>>,
		) -> Propagation {
			let (source, drop) = &mut *source.0.lock().expect("unreachable");
			let cache = &mut *cache.0.lock().expect("unreachable");
			cache.take().map(drop);
			*cache = Some(source());
			Propagation::Halt
		}
		Some(eval)
	};

	const ON_SUBSCRIBED_CHANGE: Option<
		fn(
			source: Pin<
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
impl<T: Send, S: Send + FnMut() -> T, D: Send + FnMut(T), SR: SignalsRuntimeRef>
	RawEffect<T, S, D, SR>
{
	unsafe fn init<'a>(
		source: Pin<&'a ForceSyncUnpin<Mutex<(S, D)>>>,
		cache: Slot<'a, ForceSyncUnpin<Mutex<Option<T>>>>,
	) -> Token<'a> {
		cache.write(ForceSyncUnpin(
			Some(source.project_ref().0.lock().expect("unreachable").0()).into(),
		))
	}

	pub fn pull(self: Pin<&RawEffect<T, S, D, SR>>) {
		self.0.clone_runtime_ref().run_detached(|| unsafe {
			let signal = Pin::new_unchecked(&self.0);
			signal.subscribe();
			signal.clone_runtime_ref().run_detached(|| {
				signal.project_or_init::<E>(|source, cache| {
					RawEffect::<T, S, D, SR>::init(source, cache)
				})
			});
		})
	}
}
