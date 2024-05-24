use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::Mutex,
};

use once_cell::unsync::Lazy;

use crate::SourceId;

static DIRTY_QUEUE: Mutex<Lazy<BTreeSet<SourceId>>> = Mutex::new(Lazy::new(|| BTreeSet::new()));
static CURRENT: Mutex<Option<SourceId>> = Mutex::new(None);

static INTERDEPENDENCIES: Mutex<Lazy<Interdependencies>> =
    Mutex::new(Lazy::new(|| Interdependencies::default()));

#[derive(Debug, Default)]
struct Interdependencies {
    by_dependent: HashMap<SourceId, HashSet<SourceId>>,
    by_dependency: HashMap<SourceId, HashSet<SourceId>>,
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

        dirty_queue.extend(dependents);
        let mut current = dirty_queue.pop_first().expect("unreachable");
        loop {
            *CURRENT.lock().expect("infallible") = Some(current);

            todo!();

            match DIRTY_QUEUE.lock().expect("infallible").pop_first() {
                Some(next) => current = next,
                None => break,
            }
        }
        *CURRENT.lock().expect("infallible") = None;
    }
    drop(dirty_queue);
}
