//! `Source` <=> `Future` adapters.

use std::{
    marker::PhantomData,
    mem::{self, MaybeUninit},
    pin::Pin,
    sync::Arc,
};

use async_lock::OnceCell;
use flourish::{
    raw::Source, shadow_clone, signals_helper, SignalRuntimeRef, SourcePin as _, SubscriptionSR,
    Update,
};

//TODO: Investigate: It may be possible to also implement some of this with a potential
//      `AttachedEffect` (`Effect` that doesn't use `run_detached`), which may not require
//      `T: Copy` to avoid leaks.
//      Or not. Rather, it may be much cleaner if `Signal`s could be converted into `Subscriptions` iff exclusive.

pub async fn skip_while<'a, T: 'a + Send + Sync + Clone, SR: 'a + SignalRuntimeRef>(
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
                    if !predicate_fn_pin(&*sub.read().borrow()) {
                        once.set_blocking(()).ok();
                    }
                }
            }, drop, runtime);
        }
        once.wait().await;
    }
    sub
}

pub async fn skip_while_from_source<'a, T: 'a + Send + Sync + Copy, SR: 'a + SignalRuntimeRef>(
    source: impl 'a + Source<SR, Value = T>,
    predicate_fn_pin: impl 'a + Send + FnMut(&T) -> bool,
) -> SubscriptionSR<'a, T, SR> {
    let runtime = source.clone_runtime_ref();
    skip_while(
        move || unsafe { Pin::new_unchecked(&source) }.get(),
        predicate_fn_pin,
        runtime,
    )
    .await
}

pub async fn skip_while_from_source_cloned<
    'a,
    T: 'a + Send + Sync + Clone,
    SR: 'a + SignalRuntimeRef,
>(
    source: impl 'a + Source<SR, Value = T>,
    predicate_fn_pin: impl 'a + Send + FnMut(&T) -> bool,
) -> SubscriptionSR<'a, T, SR> {
    let runtime = source.clone_runtime_ref();
    skip_while(
        move || unsafe { Pin::new_unchecked(&source) }.get_clone(),
        predicate_fn_pin,
        runtime,
    )
    .await
}

pub async fn filter<'a, T: 'a + Send + Sync + Copy, SR: 'a + SignalRuntimeRef>(
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
                    Update::Propagate
                } else {
                    Update::Halt
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

pub async fn filter_from_source<'a, T: 'a + Send + Sync + Copy, SR: 'a + SignalRuntimeRef>(
    source: impl 'a + Source<SR, Value = T>,
    predicate_fn_pin: impl 'a + Send + FnMut(&T) -> bool,
) -> SubscriptionSR<'a, T, SR> {
    let runtime = source.clone_runtime_ref();
    filter(
        move || unsafe { Pin::new_unchecked(&source) }.get(),
        predicate_fn_pin,
        runtime,
    )
    .await
}

pub async fn flatten_some<'a, T: 'a + Send + Sync + Copy, SR: 'a + SignalRuntimeRef>(
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
                    Update::Propagate
                } else {
                    Update::Halt
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

pub async fn flatten_some_from_source<'a, T: 'a + Send + Sync + Copy, SR: 'a + SignalRuntimeRef>(
    source: impl 'a + Source<SR, Value = Option<T>>,
) -> SubscriptionSR<'a, T, SR> {
    let runtime = source.clone_runtime_ref();
    flatten_some(
        move || unsafe { Pin::new_unchecked(&source) }.get(),
        runtime,
    )
    .await
}

pub struct CancellableSlot<T> {
    _phantom: PhantomData<T>,
}

pub fn while_subscribed<'a, T: 'a + Send, SR: 'a + SignalRuntimeRef>(
    load: impl FnMut(CancellableSlot<T>),
) {
    todo!()
}
