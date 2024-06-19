use std::{
    collections::{btree_map::Entry, BTreeMap, BTreeSet},
    fmt::Debug,
    hash::Hash,
};

#[derive(Debug)]
pub(crate) struct StaleQueue<S> {
    /// TODO: When projecting something, clean it if it's stale!
    stale_queue: BTreeSet<S>,
    interdependencies: Interdependencies<S>,
    sensors: BTreeMap<S, (unsafe extern "C" fn(*const (), subscribed: bool), *const ())>,
}

pub(crate) struct SensorNotification<S> {
    pub(crate) symbol: S,
    pub(crate) callback: unsafe extern "C" fn(*const (), subscribed: bool),
    pub(crate) data: *const (),
    pub(crate) value: bool,
}

impl<S> SensorNotification<S> {
    pub(crate) fn from_sensor_option(
        symbol: S,
        sensor: Option<&(unsafe extern "C" fn(*const (), subscribed: bool), *const ())>,
        value: bool,
    ) -> Option<Self> {
        sensor.map(|&(callback, data)| Self {
            symbol,
            callback,
            data,
            value,
        })
    }
}

unsafe impl<S> Send for StaleQueue<S> {}

impl<S> Default for StaleQueue<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> StaleQueue<S> {
    pub(crate) const fn new() -> Self {
        Self {
            stale_queue: BTreeSet::new(),
            interdependencies: Interdependencies::new(),
            sensors: BTreeMap::new(),
        }
    }
}

#[derive(Debug)]
struct Interdependencies<S> {
    /// Note: While a symbol is flagged as subscribed explicitly,
    ///       it is present as its own subscriber here (by not in `all_by_dependency`!).
    /// FIXME: This could store subscriber counts instead.
    subscribers_by_dependency: BTreeMap<S, BTreeSet<S>>,
    all_by_dependent: BTreeMap<S, BTreeSet<S>>,
    all_by_dependency: BTreeMap<S, BTreeSet<S>>,
}

impl<S> Interdependencies<S> {
    pub(crate) const fn new() -> Self {
        Self {
            subscribers_by_dependency: BTreeMap::new(),
            all_by_dependent: BTreeMap::new(),
            all_by_dependency: BTreeMap::new(),
        }
    }
}

impl<S: Hash + Ord + Copy + Debug> StaleQueue<S> {
    #[must_use]
    pub(crate) fn set_subscription(
        &mut self,
        symbol: S,
        enabled: bool,
    ) -> (bool, impl IntoIterator<Item = SensorNotification<S>>) {
        let subscribed = self
            .interdependencies
            .subscribers_by_dependency
            .get(&symbol)
            .expect("`set_subscription` can only be called between `start` and `stop`")
            .contains(&symbol);
        if enabled && !subscribed {
            (
                true,
                Self::subscribe_to_with(
                    symbol,
                    symbol,
                    &self.interdependencies.all_by_dependent,
                    &mut self.interdependencies.subscribers_by_dependency,
                    &self.sensors,
                )
                .into_iter()
                .collect(),
            )
        } else if !enabled && subscribed {
            (
                true,
                Self::unsubscribe_from_with(
                    symbol,
                    symbol,
                    &self.interdependencies.all_by_dependent,
                    &mut self.interdependencies.subscribers_by_dependency,
                    &self.sensors,
                )
                .into_iter()
                .collect(),
            )
        } else {
            (false, Vec::new())
        }
    }

    pub(crate) fn register_id(&mut self, symbol: S) {
        match (
            self.interdependencies
                .subscribers_by_dependency
                .entry(symbol),
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

    #[must_use]
    pub(crate) fn update_dependencies(
        &mut self,
        symbol: S,
        new_dependencies: BTreeSet<S>,
    ) -> impl IntoIterator<Item = SensorNotification<S>> {
        let old_dependencies = self
            .interdependencies
            .all_by_dependent
            .get(&symbol)
            .expect("unreachable");
        let added_dependencies = &new_dependencies - old_dependencies;
        let removed_dependencies = old_dependencies - &new_dependencies;

        drop(
            self.interdependencies
                .all_by_dependent
                .insert(symbol, new_dependencies)
                .expect("old_dependencies"),
        );
        for removed_dependency in removed_dependencies.iter() {
            assert!(self
                .interdependencies
                .all_by_dependency
                .get_mut(removed_dependency)
                .expect("unreachable")
                .remove(&symbol))
        }
        for added_dependency in added_dependencies.iter() {
            assert!(self
                .interdependencies
                .all_by_dependency
                .get_mut(added_dependency)
                .expect("unreachable")
                .insert(symbol))
        }

        let is_subscribed = !self
            .interdependencies
            .subscribers_by_dependency
            .get(&symbol)
            .expect("unreachable")
            .is_empty();
        if is_subscribed {
            removed_dependencies
                .into_iter()
                .flat_map(|removed_dependency| {
                    Self::unsubscribe_from_with(
                        removed_dependency,
                        symbol,
                        &self.interdependencies.all_by_dependent,
                        &mut self.interdependencies.subscribers_by_dependency,
                        &self.sensors,
                    )
                })
                .collect::<Vec<_>>()
                .into_iter()
                .chain(added_dependencies.into_iter().flat_map(|added_dependency| {
                    Self::subscribe_to_with(
                        added_dependency,
                        symbol,
                        &self.interdependencies.all_by_dependent,
                        &mut self.interdependencies.subscribers_by_dependency,
                        &self.sensors,
                    )
                }))
                .collect()
        } else {
            Vec::new()
        }
    }

    #[must_use]
    fn subscribe_to_with(
        dependency: S,
        dependent: S,
        all_by_dependent: &BTreeMap<S, BTreeSet<S>>,
        subscribers_by_dependency: &mut BTreeMap<S, BTreeSet<S>>,
        sensors: &BTreeMap<S, (unsafe extern "C" fn(*const (), subscribed: bool), *const ())>,
    ) -> impl IntoIterator<Item = SensorNotification<S>> {
        println!("to {:?} with {:?}", dependency, dependent);
        let subscribers = subscribers_by_dependency
            .get_mut(&dependency)
            .expect("unreachable");
        let newly_subscribed = subscribers.is_empty();
        assert!(subscribers.insert(dependent));
        if newly_subscribed {
            SensorNotification::from_sensor_option(dependency, sensors.get(&dependency), true)
                .into_iter()
                .chain(
                    all_by_dependent
                        .get(&dependency)
                        .expect("unreachable")
                        .iter()
                        .copied()
                        .flat_map(|indirect_dependency| {
                            Self::subscribe_to_with(
                                indirect_dependency,
                                dependency,
                                all_by_dependent,
                                subscribers_by_dependency,
                                sensors,
                            )
                        }),
                )
                .collect()
        } else {
            Vec::new()
        }
    }

    #[must_use]
    fn unsubscribe_from_with(
        dependency: S,
        dependent: S,
        all_by_dependent: &BTreeMap<S, BTreeSet<S>>,
        subscribers_by_dependency: &mut BTreeMap<S, BTreeSet<S>>,
        sensors: &BTreeMap<S, (unsafe extern "C" fn(*const (), subscribed: bool), *const ())>,
    ) -> impl IntoIterator<Item = SensorNotification<S>> {
        println!("from {:?} with {:?}", dependency, dependent);
        let subscribers = subscribers_by_dependency
            .get_mut(&dependency)
            .expect("unreachable");
        assert!(subscribers.remove(&dependent));
        let newly_unsubscribed = subscribers.is_empty();
        if newly_unsubscribed {
            SensorNotification::from_sensor_option(dependency, sensors.get(&dependency), false)
                .into_iter()
                .chain(
                    all_by_dependent
                        .get(&dependency)
                        .expect("unreachable")
                        .iter()
                        .copied()
                        .flat_map(|indirect_dependency| {
                            Self::unsubscribe_from_with(
                                indirect_dependency,
                                dependency,
                                all_by_dependent,
                                subscribers_by_dependency,
                                sensors,
                            )
                        }),
                )
                .collect()
        } else {
            Vec::new()
        }
    }

    pub(crate) fn mark_dependents_as_stale(&mut self, symbol: S) {
        fn mark_dependents_as_stale<S: Hash + Ord + Copy>(
            symbol: S,
            all_by_dependency: &BTreeMap<S, BTreeSet<S>>,
            stale_queue: &mut BTreeSet<S>,
        ) {
            for &dependent in all_by_dependency.get(&symbol).expect("unreachable") {
                if stale_queue.insert(dependent) {
                    mark_dependents_as_stale(dependent, all_by_dependency, stale_queue)
                }
            }
        }

        mark_dependents_as_stale(
            symbol,
            &self.interdependencies.all_by_dependency,
            &mut self.stale_queue,
        )
    }

    pub(crate) fn remove_stale(&mut self, symbol: S) -> bool {
        self.stale_queue.remove(&symbol)
    }

    #[must_use]
    pub(crate) fn purge_id(
        &mut self,
        symbol: S,
    ) -> impl IntoIterator<Item = SensorNotification<S>> {
        let is_subscribed = !self
            .interdependencies
            .subscribers_by_dependency
            .get(&symbol)
            .expect("unreachable")
            .is_empty();
        let notifications = if is_subscribed {
            self.interdependencies
                .all_by_dependent
                .get(&symbol)
                .expect("unreachable")
                .iter()
                .copied()
                .flat_map(|dependency| {
                    Self::unsubscribe_from_with(
                        dependency,
                        symbol,
                        &self.interdependencies.all_by_dependent,
                        &mut self.interdependencies.subscribers_by_dependency,
                        &self.sensors,
                    )
                })
                .collect()
        } else {
            Vec::new()
        };

        self.stale_queue.remove(&symbol);
        for map in [
            &mut self.interdependencies.subscribers_by_dependency,
            &mut self.interdependencies.all_by_dependent,
            &mut self.interdependencies.all_by_dependency,
        ] {
            map.remove(&symbol);
            for value in map.values_mut() {
                value.remove(&symbol);
            }
        }

        notifications
    }

    pub(crate) fn start_sensor(
        &mut self,
        symbol: S,
        on_subscription_change: unsafe extern "C" fn(*const (), subscribed: bool),
        on_subscription_change_data: *const (),
    ) {
        let has_subscribers = self
            .interdependencies
            .subscribers_by_dependency
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

    #[must_use]
    pub(crate) fn stop_sensor(
        &mut self,
        symbol: S,
        sensor_stack: &[S],
    ) -> impl IntoIterator<Item = SensorNotification<S>> {
        if sensor_stack.contains(&symbol) {
            //TODO: Does this need to abort the process?
            panic!("Can't stop symbol sensor while it is executing on the same thread.");
        }
        if let Some(old_sensor) = self.sensors.remove(&symbol) {
            self.interdependencies
                .subscribers_by_dependency
                .get(&symbol)
                .is_some_and(|subs| !subs.is_empty())
                .then_some(SensorNotification {
                    symbol,
                    callback: old_sensor.0,
                    data: old_sensor.1,
                    value: false,
                })
        } else {
            None
        }
    }
}

impl<S: Copy + Ord> Iterator for StaleQueue<S> {
    type Item = S;

    fn next(&mut self) -> Option<Self::Item> {
        //FIXME: This is very inefficient! Stale-marking propagates only forwards, so one step up in the call graph, a cursor can be used.
        let next = self.stale_queue.iter().copied().find(|next| {
            !self
                .interdependencies
                .subscribers_by_dependency
                .get(&next)
                .expect("unreachable")
                .is_empty()
        });
        if let Some(next) = next {
            assert!(self.stale_queue.remove(&next));
        }
        next
    }
}
