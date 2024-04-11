use std::{collections::HashMap, num::NonZeroU64, sync::Mutex};

use once_cell::unsync::Lazy;
use range_set_blaze::RangeSetBlaze;

use crate::SourceId;

static DIRTY_QUEUE: Mutex<Lazy<RangeSetBlaze<u64>>> =
    Mutex::new(Lazy::new(|| RangeSetBlaze::new()));
static CURRENT: Mutex<Option<SourceId>> = Mutex::new(None);

static INTERDEPENDENCIES: Mutex<Lazy<Interdependencies>> =
    Mutex::new(Lazy::new(|| Interdependencies::default()));

#[derive(Debug, Default)]
struct Interdependencies {
    by_dependent: HashMap<SourceId, RangeSetBlaze<u64>>,
    by_dependency: HashMap<SourceId, RangeSetBlaze<u64>>,
}

fn to_source_id(u64: u64) -> SourceId {
    SourceId(NonZeroU64::new(u64).expect("unreachable"))
}

pub(crate) fn eval_dependents(dependency: SourceId) {
    let mut dirty_queue = DIRTY_QUEUE
        .try_lock()
        .expect("should be synchronised by work queue");

    if let Some(dependents) = INTERDEPENDENCIES
        .lock()
        .expect("infallible")
        .by_dependency
        .get(&dependency)
    {
        if dependents.is_empty() {
            return;
        };

        **dirty_queue |= dependents;
        let mut current = to_source_id(dirty_queue.pop_first().expect("unreachable"));
        loop {
            *CURRENT.lock().expect("infallible") = Some(current);

            todo!();

            match DIRTY_QUEUE
                .lock()
                .expect("infallible")
                .pop_first()
                .map(to_source_id)
            {
                Some(next) => current = next,
                None => break,
            }
        }
        *CURRENT.lock().expect("infallible") = None;
    }
    drop(dirty_queue);
}
