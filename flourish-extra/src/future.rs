//! `Source` <=> `Future` adapters.

use async_lock::OnceCell;
use flourish::{signals_helper, SignalRuntimeRef, Source, SubscriptionSR};

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
