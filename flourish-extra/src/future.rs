//! `Source` <=> `Future` adapters.

use std::{
    mem::{self, MaybeUninit},
    pin::Pin,
    sync::Arc,
};

use async_lock::OnceCell;
use flourish::{shadow_clone, signals_helper, SignalRuntimeRef, Source, SubscriptionSR, Update};

pub async fn skipped_while<'a, T: 'a + Send + Sync + Copy, SR: 'a + SignalRuntimeRef>(
    source: impl 'a + Source<SR, Value = T>,
    mut test: impl 'a + Send + FnMut(&T) -> bool,
) -> SubscriptionSR<'a, T, SR> {
    let runtime = source.clone_runtime_ref();
    let sub = SubscriptionSR::new(source);
    {
        let once = OnceCell::<()>::new();
        signals_helper! {
            let effect = effect_with_runtime!({
                let (sub, once) = (&sub, &once);
                move || {
                    if !test(&*sub.read().borrow()) {
                        once.set_blocking(()).ok();
                    }
                }
            }, drop, runtime);
        }
        once.wait().await;
    }
    sub
}

pub async fn skipped_while_cloned<'a, T: 'a + Send + Sync + Clone, SR: 'a + SignalRuntimeRef>(
    source: impl 'a + Source<SR, Value = T>,
    mut test: impl 'a + Send + FnMut(&T) -> bool,
) -> SubscriptionSR<'a, T, SR> {
    let runtime = source.clone_runtime_ref();
    let sub = SubscriptionSR::new(source);
    {
        let once = OnceCell::<()>::new();
        signals_helper! {
            let effect = effect_with_runtime!({
                let (sub, once) = (&sub, &once);
                move || {
                    if !test(&*sub.read().borrow()) {
                        once.set_blocking(()).ok();
                    }
                }
            }, drop, runtime);
        }
        once.wait().await;
    }
    sub
}

pub async fn filtered<'a, T: 'a + Send + Sync + Copy, SR: 'a + SignalRuntimeRef>(
    source: impl 'a + Source<SR, Value = T>,
    mut test: impl 'a + Send + FnMut(&T) -> bool,
) -> SubscriptionSR<'a, T, SR> {
    let runtime = source.clone_runtime_ref();
    let once = Arc::new(OnceCell::<()>::new());
    let sub = SubscriptionSR::folded_with_runtime(
        MaybeUninit::uninit(),
        {
            shadow_clone!(once);
            move |value| {
                let next = unsafe { Pin::new_unchecked(&source) }.get();
                if test(&next) {
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
        mem::transmute::<SubscriptionSR<'a, MaybeUninit<T>, SR>, SubscriptionSR<'a, T, SR>>(sub)
    }
}
