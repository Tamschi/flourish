//! `Source` <=> `Future` adapters.

use std::{
	marker::PhantomData,
	mem::{self, MaybeUninit},
	sync::Arc,
};

use async_lock::OnceCell;
use flourish::{
	prelude::*, shadow_clone, signals_helper, Propagation, SignalsRuntimeRef, SubscriptionSR,
};

pub async fn skipped_while<'a, T: 'a + Send + Sync, SR: 'a + SignalsRuntimeRef>(
	fn_pin: impl 'a + Send + FnMut() -> T,
	mut predicate_fn_pin: impl 'a + Send + FnMut(&T) -> bool,
	runtime: SR,
) -> SubscriptionSR<'a, T, SR> {
	let sub = SubscriptionSR::computed_with_runtime(fn_pin, runtime.clone());
	{
		let once = OnceCell::<()>::new();
		signals_helper! {
			let effect = effect_with_runtime!({
				let (sub, once) = (&sub, &once);
				move || {
					if !predicate_fn_pin(&*sub.read_dyn().borrow()) {
						once.set_blocking(()).ok();
					}
				}
			}, drop, runtime);
		}
		once.wait().await;
	}
	sub
}

pub async fn filtered<'a, T: 'a + Send + Sync + Copy, SR: 'a + SignalsRuntimeRef>(
	mut fn_pin: impl 'a + Send + FnMut() -> T,
	mut predicate_fn_pin: impl 'a + Send + FnMut(&T) -> bool,
	runtime: SR,
) -> SubscriptionSR<'a, T, SR> {
	// It's actually possible to avoid the `Arc` here, with a tri-state atomic or another `Once`,
	// since the closure is guaranteed to run when the subscription is created.
	// However, that would be considerably trickier code.
	let once = Arc::new(OnceCell::<()>::new());
	let sub = SubscriptionSR::folded_with_runtime(
		MaybeUninit::uninit(),
		{
			shadow_clone!(once);
			move |value| {
				let next = fn_pin();
				if predicate_fn_pin(&next) {
					if once.is_initialized() {
						*unsafe { value.assume_init_mut() } = next;
					} else {
						value.write(next);
						once.set_blocking(()).expect("unreachable");
					}
					Propagation::Propagate
				} else {
					Propagation::Halt
				}
			}
		},
		runtime,
	);
	once.wait().await;

	unsafe {
		//SAFETY: This is fine because `dyn Source` is ABI-compatible across ABI-compatible `Value`s by definition.
		//CORRECTNESS: This neglects to call `T::drop()`, but that's fine because `T: Copy`.
		mem::transmute::<SubscriptionSR<'a, MaybeUninit<T>, SR>, SubscriptionSR<'a, T, SR>>(sub)
	}
}

pub async fn filter_mapped<'a, T: 'a + Send + Sync + Copy, SR: 'a + SignalsRuntimeRef>(
	mut fn_pin: impl 'a + Send + FnMut() -> Option<T>,
	runtime: SR,
) -> SubscriptionSR<'a, T, SR> {
	// It's actually possible to avoid the `Arc` here, with a tri-state atomic or another `Once`,
	// since the closure is guaranteed to run when the subscription is created.
	// However, that would be considerably trickier code.
	let once = Arc::new(OnceCell::<()>::new());
	let sub = SubscriptionSR::folded_with_runtime(
		MaybeUninit::uninit(),
		{
			shadow_clone!(once);
			move |value| {
				if let Some(next) = fn_pin() {
					if once.is_initialized() {
						*unsafe { value.assume_init_mut() } = next;
					} else {
						value.write(next);
						once.set_blocking(()).expect("unreachable");
					}
					Propagation::Propagate
				} else {
					Propagation::Halt
				}
			}
		},
		runtime,
	);
	once.wait().await;

	unsafe {
		//SAFETY: This is fine because `dyn Source` is ABI-compatible across ABI-compatible `Value`s by definition.
		//CORRECTNESS: This neglects to call `T::drop()`, but that's fine because `T: Copy`.
		mem::transmute::<SubscriptionSR<'a, MaybeUninit<T>, SR>, SubscriptionSR<'a, T, SR>>(sub)
	}
}

pub struct CancellableSlot<T> {
	_phantom: PhantomData<T>,
}

pub fn while_subscribed<'a, T: 'a + Send, SR: 'a + SignalsRuntimeRef>(
	load: impl FnMut(CancellableSlot<T>),
) {
	todo!()
}
