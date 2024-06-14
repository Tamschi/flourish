use std::{
    collections::{btree_map::Entry, BTreeMap, BTreeSet, VecDeque},
    hash::Hash,
};

use crate::source::SourceId;

#[derive(Debug)]
pub(crate) struct DirtyQueue<S> {
    dirty_queue: BTreeSet<S>,
    current: Option<S>,
    interdependencies: Interdependencies<S>,
    sensors: BTreeMap<S, (unsafe extern "C" fn(*const (), subscribed: bool), *const ())>,
    sensor_stack: VecDeque<S>,
}

unsafe impl<S> Send for DirtyQueue<S> {}

impl<S> Default for DirtyQueue<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> DirtyQueue<S> {
    pub(crate) const fn new() -> Self {
        Self {
            dirty_queue: BTreeSet::new(),
            current: None,
            interdependencies: Interdependencies::new(),
            sensors: BTreeMap::new(),
            sensor_stack: VecDeque::new(),
        }
    }
}

#[derive(Debug)]
struct Interdependencies<S> {
    /// Note: While a symbol is flagged as subscribed explicitly,
    ///       it is present as its own dependency here (by not in `all_by_dependency`!).
    /// TODO: When cleaning dirty flags, use this to check whether to call the callback.
    ///       If a symbol lacks subscribers, then the associated callback isn't called.
    subscribed_by_dependent: BTreeMap<S, BTreeSet<S>>,
    all_by_dependency: BTreeMap<S, BTreeSet<S>>,
}

impl<S> Interdependencies<S> {
    pub(crate) const fn new() -> Self {
        Self {
            subscribed_by_dependent: BTreeMap::new(),
            all_by_dependency: BTreeMap::new(),
        }
    }
}

impl<S: Hash + Ord + Copy> DirtyQueue<S> {
    pub(crate) fn set_subscription(&mut self, symbol: S, enabled: bool) -> bool {
        let subscribed_dependencies = self
            .interdependencies
            .subscribed_by_dependent
            .get_mut(&symbol)
            .expect("`set_subscription` can only be called between `start` and `stop`");
        if enabled {
            let incremented = subscribed_dependencies.insert(symbol);
            if incremented && subscribed_dependencies.len() == 1 {
                //TODO: Propagate
                //TODO: Subscriber notification mechanism!
            }
            incremented
        } else {
            let decremented = subscribed_dependencies.remove(&symbol);
            if decremented && subscribed_dependencies.is_empty() {
                //TODO: Propagate
                //TODO: Subscriber notification mechanism!
            }
            decremented
        }
    }

    pub(crate) fn purge_id(&mut self, symbol: S) {
        if self.current == Some(symbol) {
            self.dirty_queue.remove(&symbol);
            self.interdependencies.all_by_dependency.remove(&symbol);
            for dependents in self.interdependencies.all_by_dependency.values_mut() {
                dependents.remove(&symbol);
            }
            self.interdependencies
                .subscribed_by_dependent
                .remove(&symbol);
            for dependencies in self.interdependencies.subscribed_by_dependent.values_mut() {
                dependencies.remove(&symbol);
            }
        }
    }

    pub(crate) fn start_sensor(
        &mut self,
        symbol: S,
        on_subscription_change: unsafe extern "C" fn(*const (), subscribed: bool),
        on_subscription_change_data: *const (),
    ) {
        let has_subscribers = self
            .interdependencies
            .subscribed_by_dependent
            .get(&symbol)
            .is_some_and(|subs| !subs.is_empty());

        match self.sensors.entry(symbol) {
            Entry::Vacant(e) => e.insert((on_subscription_change, on_subscription_change_data)),
            Entry::Occupied(_) => {
                panic!("For now, please call `stop_sensor` before setting up another one.")
            }
        };

        if has_subscribers {
            unsafe { on_subscription_change(on_subscription_change_data, true) }
        }
    }

    pub(crate) fn stop_sensor(&mut self, symbol: S) {
        if self.sensor_stack.contains(&symbol) {
            //TODO: Does this need to abort the process?
            panic!("Can't stop symbol sensor while it is executing on the same thread.");
        }
        if let Some(old_sensor) = self.sensors.remove(&symbol) {
            let has_subscribers = self
                .interdependencies
                .subscribed_by_dependent
                .get(&symbol)
                .is_some_and(|subs| !subs.is_empty());
            if has_subscribers {
                unsafe { old_sensor.0(old_sensor.1, false) }
            }
        }
    }
}

impl<S: Copy + Ord> Iterator for DirtyQueue<S> {
    type Item = S;

    fn next(&mut self) -> Option<Self::Item> {
        self.current = self.dirty_queue.pop_first();
        self.current
    }
}
