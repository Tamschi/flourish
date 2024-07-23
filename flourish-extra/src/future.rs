//! `Source` <=> `Future` adapters.

use std::{
	borrow::Borrow,
	marker::PhantomData,
	mem::{ManuallyDrop, MaybeUninit},
	ops::Deref,
	pin::Pin,
	sync::Arc,
};

use async_lock::OnceCell;
use flourish::{
	prelude::*,
	raw::{Source, Subscribable},
	shadow_clone, signals_helper, Guard, Propagation, SignalsRuntimeRef, SubscriptionSR,
};
use pin_project::pin_project;

pub async fn skipped_while<'a, T: 'a + Send + Sync, SR: 'a + SignalsRuntimeRef>(
	fn_pin: impl 'a + Send + FnMut() -> T,
	mut predicate_fn_pin: impl 'a + Send + FnMut(&T) -> bool,
	runtime: SR,
) -> SubscriptionSR<T, impl 'a + Subscribable<T, SR>, SR> {
	let sub = SubscriptionSR::computed_with_runtime(fn_pin, runtime.clone());
	{
		let once = OnceCell::<()>::new();
		signals_helper! {
			let effect = effect_with_runtime!({
				let (sub, once) = (&sub, &once);
				move || {
					if !predicate_fn_pin(&**sub.read_dyn()) {
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
) -> SubscriptionSR<T, impl 'a + Subscribable<T, SR>, SR> {
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

	unsafe { assume_init_subscription(sub) }
}

pub async fn filter_mapped<'a, T: 'a + Send + Sync + Copy, SR: 'a + SignalsRuntimeRef>(
	mut fn_pin: impl 'a + Send + FnMut() -> Option<T>,
	runtime: SR,
) -> SubscriptionSR<T, impl 'a + Subscribable<T, SR>, SR> {
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

	unsafe { assume_init_subscription(sub) }
}

pub struct CancellableSlot<T> {
	_phantom: PhantomData<T>,
}

pub fn while_subscribed<'a, T: 'a + Send, SR: 'a + SignalsRuntimeRef>(
	load: impl FnMut(CancellableSlot<T>),
) {
	todo!()
}

unsafe fn assume_init_subscription<
	T: ?Sized + Send + Copy,
	S: Subscribable<MaybeUninit<T>, SR>,
	SR: SignalsRuntimeRef,
>(
	sub: SubscriptionSR<MaybeUninit<T>, S, SR>,
) -> SubscriptionSR<T, impl Subscribable<T, SR>, SR> {
	#[pin_project]
	#[repr(transparent)]
	struct AbiShim<T: ?Sized>(#[pin] T);

	impl<T: Send + Copy, S: Source<MaybeUninit<T>, SR>, SR: SignalsRuntimeRef> Source<T, SR>
		for AbiShim<S>
	{
		fn touch(self: Pin<&Self>) {
			self.project_ref().0.touch()
		}

		fn get(self: Pin<&Self>) -> T
		where
			T: Sync + Copy,
		{
			unsafe { self.project_ref().0.get().assume_init() }
		}

		fn get_clone(self: Pin<&Self>) -> T
		where
			T: Sync + Clone,
		{
			unsafe { self.project_ref().0.get_clone().assume_init() }
		}

		fn get_clone_exclusive(self: Pin<&Self>) -> T
		where
			T: Clone,
		{
			unsafe { self.project_ref().0.get_clone_exclusive().assume_init() }
		}

		fn get_exclusive(self: Pin<&Self>) -> T
		where
			T: Copy,
		{
			unsafe { self.project_ref().0.get_exclusive().assume_init() }
		}

		fn read<'r>(self: Pin<&'r Self>) -> Self::Read<'r>
		where
			Self: Sized,
			T: 'r + Sync,
		{
			AbiShim(self.project_ref().0.read())
		}

		type Read<'r> = AbiShim<S::Read<'r>>
		where
			Self: 'r + Sized,
			T: 'r + Sync;

		fn read_exclusive<'r>(self: Pin<&'r Self>) -> Self::ReadExclusive<'r>
		where
			Self: Sized,
			T: 'r,
		{
			AbiShim(self.project_ref().0.read_exclusive())
		}

		type ReadExclusive<'r> = AbiShim<S::ReadExclusive<'r>>
		where
			Self: 'r + Sized,
			T: 'r;

		fn read_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Guard<T>>
		where
			T: 'r + Sync,
		{
			unsafe {
				//SAFETY: `MaybeUninit` is ABI-compatible with what it wraps.
				Box::from_raw(
					*(&Box::into_raw(self.project_ref().0.read_exclusive_dyn())
						as *const *mut dyn Guard<MaybeUninit<T>> as *const *mut dyn Guard<T>),
				)
			}
		}

		fn read_exclusive_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Guard<T>>
		where
			T: 'r,
		{
			unsafe {
				//SAFETY: `MaybeUninit` is ABI-compatible with what it wraps.
				Box::from_raw(
					*(&Box::into_raw(self.project_ref().0.read_exclusive_dyn())
						as *const *mut dyn Guard<MaybeUninit<T>> as *const *mut dyn Guard<T>),
				)
			}
		}

		fn clone_runtime_ref(&self) -> SR
		where
			SR: Sized,
		{
			self.0.clone_runtime_ref()
		}
	}

	impl<T: Send + Copy, S: Subscribable<MaybeUninit<T>, SR>, SR: SignalsRuntimeRef>
		Subscribable<T, SR> for AbiShim<S>
	{
		fn subscribe_inherently(self: Pin<&Self>) -> bool {
			self.project_ref().0.subscribe_inherently()
		}

		fn unsubscribe_inherently(self: Pin<&Self>) -> bool {
			self.project_ref().0.unsubscribe_inherently()
		}
	}

	impl<T: ?Sized + Send + Copy, G: ?Sized + Guard<MaybeUninit<T>>> Guard<T> for AbiShim<G> {}

	impl<T: ?Sized + Send + Copy, G: ?Sized + Deref<Target = MaybeUninit<T>>> Deref for AbiShim<G> {
		type Target = T;

		fn deref(&self) -> &Self::Target {
			unsafe { self.0.deref().assume_init_ref() }
		}
	}

	impl<T: ?Sized + Send + Copy, G: ?Sized + Borrow<MaybeUninit<T>>> Borrow<T> for AbiShim<G> {
		fn borrow(&self) -> &T {
			unsafe { self.0.borrow().assume_init_ref() }
		}
	}

	unsafe {
		//SAFETY: This may reinterpret a fat pointer, which skips over the `AbiShim` methods
		//        entirely, but that's fine since everything is fully ABI-compatible.
		(*(&(&ManuallyDrop::new(sub) as *const ManuallyDrop<SubscriptionSR<MaybeUninit<T>, S, SR>>)
			as *const *const ManuallyDrop<SubscriptionSR<MaybeUninit<T>, S, SR>>
			as *const *const SubscriptionSR<T, AbiShim<S>, SR>))
			.read()
	}
}
