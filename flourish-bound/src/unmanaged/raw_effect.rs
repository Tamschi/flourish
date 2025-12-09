use std::{cell::RefCell, pin::Pin};

use isoprenoid_bound::{
	raw::{Callbacks, RawSignal},
	runtime::{CallbackTableTypes, Propagation, SignalsRuntimeRef},
	slot::{Slot, Token},
};

#[must_use = "Effects are cancelled when dropped."]
#[repr(transparent)]
pub struct RawEffect<T, S: FnMut() -> T, D: FnMut(T), SR: SignalsRuntimeRef>(
	RawSignal<RefCell<(S, D)>, RefCell<Option<T>>, SR>,
);

//TODO: Add some associated methods, like not-boxing `read`.
//TODO: Turn some of these functions into methods.

#[doc(hidden)]
pub fn new_raw_unsubscribed_effect<T, S: FnMut() -> T, D: FnMut(T), SR: SignalsRuntimeRef>(
	init_fn_pin: S,
	drop_fn_pin: D,
	runtime: SR,
) -> RawEffect<T, S, D, SR> {
	RawEffect(RawSignal::with_runtime(
		(init_fn_pin, drop_fn_pin).into(),
		runtime,
	))
}

impl<T, S: FnMut() -> T, D: FnMut(T), SR: SignalsRuntimeRef> Drop for RawEffect<T, S, D, SR> {
	fn drop(&mut self) {
		let raw_signal = unsafe { Pin::new_unchecked(&mut self.0) };
		raw_signal.purge_and_deinit_with(|eager, lazy| {
			let drop = &mut eager.borrow_mut().1;
			if let Some(value) = lazy.borrow_mut().take() {
				drop(value)
			}
		});
	}
}

enum E {}
impl<T, S: FnMut() -> T, D: FnMut(T), SR: SignalsRuntimeRef>
	Callbacks<RefCell<(S, D)>, RefCell<Option<T>>, SR> for E
{
	const UPDATE: Option<
		fn(eager: Pin<&RefCell<(S, D)>>, lazy: Pin<&RefCell<Option<T>>>) -> Propagation,
	> = {
		fn eval<T, S: FnMut() -> T, D: FnMut(T)>(
			source: Pin<&RefCell<(S, D)>>,
			cache: Pin<&RefCell<Option<T>>>,
		) -> Propagation {
			let (source, drop) = &mut *source.borrow_mut();

			//TODO: `cache.update` is likely the call, but it seems isoprenoid-bound must be fixed to use intellisense.
			let cache = &mut *cache.borrow_mut();
			cache.take().map(drop);
			*cache = Some(source());

			Propagation::Halt
		}
		Some(eval)
	};

	const ON_SUBSCRIBED_CHANGE: Option<
		fn(
			source: Pin<&RawSignal<RefCell<(S, D)>, RefCell<Option<T>>, SR>>,
			eager: Pin<&RefCell<(S, D)>>,
			lazy: Pin<&RefCell<Option<T>>>,
			subscribed: <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
		) -> Propagation,
	> = None;
}

/// # Safety
///
/// These are the only functions that access `cache`.
/// Externally synchronised through guarantees on [`isoprenoid_bound::raw::Callbacks`].
impl<T, S: FnMut() -> T, D: FnMut(T), SR: SignalsRuntimeRef> RawEffect<T, S, D, SR> {
	unsafe fn init<'a>(
		source: Pin<&'a RefCell<(S, D)>>,
		cache: Slot<'a, RefCell<Option<T>>>,
	) -> Token<'a> {
		cache.write(Some(source.borrow_mut().0()).into())
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
