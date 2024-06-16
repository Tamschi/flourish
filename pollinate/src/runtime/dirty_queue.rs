use std::{
    collections::{btree_map::Entry, BTreeMap, BTreeSet, VecDeque},
    convert::identity,
    hash::Hash,
};

use crate::runtime::dirty_queue;

#[derive(Debug)]
pub(crate) struct DirtyQueue<S> {
    dirty_queue: BTreeSet<S>,
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
    all_by_dependent: BTreeMap<S, BTreeSet<S>>,
    all_by_dependency: BTreeMap<S, BTreeSet<S>>,
}

impl<S> Interdependencies<S> {
    pub(crate) const fn new() -> Self {
        Self {
            subscribed_by_dependent: BTreeMap::new(),
            all_by_dependent: BTreeMap::new(),
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

    pub(crate) fn register_id(&mut self, symbol: S) {
        match (
            self.interdependencies.subscribed_by_dependent.entry(symbol),
            self.interdependencies.all_by_dependent.entry(symbol),
            self.interdependencies.all_by_dependency.entry(symbol),
        ) {
            (Entry::Vacant(v1), Entry::Vacant(v2), Entry::Vacant(v3)) => {
                v1.insert(BTreeSet::new());
                v2.insert(BTreeSet::new());
                v3.insert(BTreeSet::new());
            }
            (_, _, _) => {
                panic!("Tried to `register_id` twice without calling `purge_id` in-between.")
            }
        }
    }

    pub(crate) fn update_dependencies(&mut self, symbol: S, new_dependencies: BTreeSet<S>) {
        let old_dependencies = self
            .interdependencies
            .all_by_dependent
            .get(&symbol)
            .expect("unreachable");
        let added_dependencies = &new_dependencies - old_dependencies;
        let removed_dependencies = old_dependencies - &new_dependencies;

        let was_subscribed = !self
            .interdependencies
            .subscribed_by_dependent
            .get(&symbol)
            .expect("unreachable")
            .is_empty();
        let new_subscribed_dependencies: BTreeSet<S> = new_dependencies
            .iter()
            .copied()
            .filter(|d| {
                !self
                    .interdependencies
                    .subscribed_by_dependent
                    .get(d)
                    .expect("unreachable")
                    .is_empty()
            })
            .collect();
        let is_subscribed = !new_subscribed_dependencies.is_empty();
        drop(
            self.interdependencies
                .subscribed_by_dependent
                .insert(symbol, new_subscribed_dependencies),
        );
        drop(
            self.interdependencies
                .all_by_dependent
                .insert(symbol, new_dependencies)
                .expect("old_dependencies"),
        );
        for removed_dependency in removed_dependencies {
            assert!(self
                .interdependencies
                .all_by_dependency
                .get_mut(&removed_dependency)
                .expect("unreachable")
                .remove(&symbol));
            if was_subscribed {
                let subscribed_of_dependency = &mut self
                    .interdependencies
                    .subscribed_by_dependent
                    .get_mut(&removed_dependency)
                    .expect("unreachable");
                assert!(subscribed_of_dependency.remove(&symbol));
                if subscribed_of_dependency.is_empty() {
                    //TODO: Propagate!
                    //TODO: Notify!
                }
            }
        }
        if was_subscribed && !is_subscribed {
            //TODO: Propagate!
        }
        if !was_subscribed && is_subscribed {
            //TODO: Propagate!
        }
        for added_dependency in added_dependencies {
            assert!(self
                .interdependencies
                .all_by_dependency
                .get_mut(&added_dependency)
                .expect("unreachable")
                .insert(symbol));
            if is_subscribed {
                let subscribed_of_dependency = &mut self
                    .interdependencies
                    .subscribed_by_dependent
                    .get_mut(&added_dependency)
                    .expect("unreachable");
                assert!(subscribed_of_dependency.insert(symbol));
                if subscribed_of_dependency.len() == 1 {
                    //TODO: Propagate!
                    //TODO: Notify!
                }
            }
        }
    }

    pub(crate) fn mark_dependents_as_dirty(&mut self, symbol: S) {
        fn mark_dependents_as_dirty<S: Hash + Ord + Copy>(
            symbol: S,
            all_by_dependency: &BTreeMap<S, BTreeSet<S>>,
            dirty_queue: &mut BTreeSet<S>,
        ) {
            for &dependent in all_by_dependency.get(&symbol).expect("unreachable") {
                if dirty_queue.insert(dependent) {
                    mark_dependents_as_dirty(dependent, all_by_dependency, dirty_queue)
                }
            }
        }

        mark_dependents_as_dirty(
            symbol,
            &self.interdependencies.all_by_dependency,
            &mut self.dirty_queue,
        )
    }

    pub(crate) fn purge_id(&mut self, symbol: S) {
        self.dirty_queue.remove(&symbol);
        for map in [
            &mut self.interdependencies.subscribed_by_dependent,
            &mut self.interdependencies.all_by_dependent,
            &mut self.interdependencies.all_by_dependency,
        ] {
            map.remove(&symbol);
            for value in map.values_mut() {
                value.remove(&symbol);
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
        let next = self.dirty_queue.iter().copied().find(|next| {
            !self
                .interdependencies
                .subscribed_by_dependent
                .get(&next)
                .expect("unreachable")
                .is_empty()
        });
        if let Some(next) = next {
            assert!(self.dirty_queue.remove(&next));
        }
        next
    }
}
