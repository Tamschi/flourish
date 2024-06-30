use core::{
    fmt::Debug,
    num::NonZeroU64,
    sync::atomic::{AtomicU64, Ordering},
};
use std::{
    cell::{RefCell, RefMut},
    collections::{btree_map::Entry, BTreeMap, BTreeSet, VecDeque},
    mem,
    panic::{catch_unwind, resume_unwind, AssertUnwindSafe},
    sync::{Arc, Barrier, Mutex, MutexGuard},
    thread,
};

use parking_lot::{ReentrantMutex, ReentrantMutexGuard};
use stale_queue::{SensorNotification, StaleQueue};

mod deferred_queue;
mod stale_queue;

pub trait SignalRuntimeRef: Send + Sync + Clone {
    type Symbol: Clone + Copy + Send;
    fn next_id(&self) -> Self::Symbol;
    fn reentrant_critical<T>(&self, f: impl FnOnce() -> T) -> T;
    fn touch(&self, id: Self::Symbol);
    unsafe fn start<T, D: ?Sized>(
        &self,
        id: Self::Symbol,
        f: impl FnOnce() -> T,
        callback_table: *const CallbackTable<D>,
        callback_data: *const D,
    ) -> T;
    fn update_dependency_set<T>(&self, id: Self::Symbol, f: impl FnOnce() -> T) -> T;
    /// # Returns
    ///
    /// Whether there was a change.
    fn set_subscription(&self, id: Self::Symbol, enabled: bool) -> bool;
    fn update_or_enqueue(&self, id: Self::Symbol, f: impl 'static + Send + FnOnce());
    fn update_or_enqueue_blocking(&self, id: Self::Symbol, f: impl FnOnce());
    fn propagate_from(&self, id: Self::Symbol);
    fn run_detached<T>(&self, f: impl FnOnce() -> T) -> T;
    fn refresh(&self, id: Self::Symbol);
    fn stop(&self, id: Self::Symbol);
}

#[derive(Default)]
struct ASignalRuntime {
    source_counter: AtomicU64,
    /// Overlapping locks are always in the order `critical_mutex`->`update_queue`.
    critical_mutex: ReentrantMutex<RefCell<ASignalRuntime_>>,
    /// Overlapping locks are always in the order `critical_mutex`->`update_queue`.
    update_queue: Mutex<VecDeque<(ASymbol, Box<dyn 'static + Send + FnOnce()>)>>,
}

unsafe impl Sync for ASignalRuntime {}

#[derive(Default)]
struct ASignalRuntime_ {
    stale_queue: StaleQueue<ASymbol>,
    context_stack: Vec<Option<(ASymbol, BTreeSet<ASymbol>)>>,
    callbacks: BTreeMap<ASymbol, (*const CallbackTable<()>, *const ())>,
}

impl ASignalRuntime_ {
    const fn new() -> Self {
        Self {
            stale_queue: StaleQueue::new(),
            context_stack: Vec::new(),
            callbacks: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct ASymbol(NonZeroU64);

impl ASignalRuntime {
    const fn new() -> Self {
        Self {
            source_counter: AtomicU64::new(0),
            critical_mutex: ReentrantMutex::new(RefCell::new(ASignalRuntime_::new())),
            update_queue: Mutex::new(VecDeque::new()),
        }
    }

    #[must_use]
    fn notify_all<'a: 'b, 'b>(
        lock: &'a ReentrantMutexGuard<RefCell<ASignalRuntime_>>,
        notifications: impl IntoIterator<Item = SensorNotification<ASymbol>>,
        mut borrow: RefMut<'b, ASignalRuntime_>,
    ) -> RefMut<'b, ASignalRuntime_> {
        fn notify<'a: 'b, 'b>(
            lock: &'a ReentrantMutexGuard<RefCell<ASignalRuntime_>>,
            SensorNotification { symbol, value }: SensorNotification<ASymbol>,
            mut borrow: RefMut<'b, ASignalRuntime_>,
        ) -> RefMut<'b, ASignalRuntime_> {
            let &(callback_table, data) = borrow.callbacks.get(&symbol).expect("unreachable");
            if let &CallbackTable {
                on_subscribed_change: Some(on_subscribed_change),
                ..
            } = unsafe { &*callback_table }
            {
                //TODO: Dirty queue isolation!
                borrow.context_stack.push(None); // Important! Dependency isolation.
                drop(borrow);
                let r = catch_unwind(|| unsafe { on_subscribed_change(data, value) });
                let mut borrow = (*lock).borrow_mut();
                assert_eq!(borrow.context_stack.pop(), Some(None));
                if let Err(payload) = r {
                    resume_unwind(payload)
                }
                borrow
            } else {
                borrow
            }
        }

        for notification in notifications {
            borrow = notify(&lock, notification, borrow)
        }
        borrow
    }

    #[must_use = "The guard drop order is important! Drop the reentrant guard first if you have one."]
    fn process_updates_if_ready<'a>(
        &'a self,
        mut update_queue: MutexGuard<'a, VecDeque<(ASymbol, Box<dyn 'static + Send + FnOnce()>)>>,
    ) -> MutexGuard<VecDeque<(ASymbol, Box<dyn 'static + Send + FnOnce()>)>> {
        let Some(critical) = self.critical_mutex.try_lock() else {
            return update_queue;
        };
        {
            let borrow = critical.borrow();
            if !borrow.context_stack.is_empty() || borrow.stale_queue.peek().is_some() {
                // Still processing something else (which will)
                return update_queue;
            }
        }

        while let Some((id, next)) = update_queue.pop_front() {
            // Still holding onto `critical`, so this code remains synchronised!
            drop(update_queue);
            let borrow = critical.borrow();
            debug_assert!(borrow.callbacks.contains_key(&id));
            drop(borrow);
            next();
            update_queue = self.update_queue.lock().expect("unreachable");
        }
        drop(critical);
        update_queue
    }
}

impl SignalRuntimeRef for &ASignalRuntime {
    type Symbol = ASymbol;

    fn next_id(&self) -> Self::Symbol {
        ASymbol(
            //TODO: Relax ordering?
            (self.source_counter.fetch_add(1, Ordering::SeqCst) + 1)
                .try_into()
                .expect("infallible within reasonable time"),
        )
    }

    fn reentrant_critical<T>(&self, f: impl FnOnce() -> T) -> T {
        let _guard = self.critical_mutex.lock();
        f()
    }

    fn touch(&self, id: Self::Symbol) {
        let lock = self.critical_mutex.lock();
        let mut borrow = (*lock).borrow_mut();
        if let Some(Some((context_id, touched))) = &mut borrow.context_stack.last_mut() {
            if id >= *context_id {
                panic!("Tried to depend on later-created signal. To prevent loops, this isn't possible for now.");
            }
            touched.insert(id);
        }
    }

    unsafe fn start<T, D: ?Sized>(
        &self,
        id: Self::Symbol,
        f: impl FnOnce() -> T,
        callback_table: *const CallbackTable<D>,
        callback_data: *const D,
    ) -> T {
        let lock = self.critical_mutex.lock();
        {
            let mut borrow = (*lock).borrow_mut();
            borrow.stale_queue.register_id(id);
            borrow.context_stack.push(Some((id, BTreeSet::new())));
        }
        let r = catch_unwind(AssertUnwindSafe(f));
        {
            let mut borrow = (*lock).borrow_mut();
            let (popped_id, touched_dependencies) =
                borrow.context_stack.pop().flatten().expect("unreachable");
            assert_eq!(popped_id, id);
            let notifications = borrow
                .stale_queue
                .update_dependency_set(id, touched_dependencies);
            match borrow.callbacks.entry(id) {
                Entry::Vacant(v) => v.insert((
                    CallbackTable::into_erased_ptr(callback_table),
                    callback_data.cast(),
                )),
                Entry::Occupied(_) => panic!("Can't call `start` again before calling `stop`."),
            };
            let _ = ASignalRuntime::notify_all(&lock, notifications, borrow);
        }
        r.unwrap_or_else(|p| resume_unwind(p))
    }

    fn update_dependency_set<T>(&self, id: Self::Symbol, f: impl FnOnce() -> T) -> T {
        let lock = self.critical_mutex.lock();
        {
            let mut borrow = (*lock).borrow_mut();
            borrow.context_stack.push(Some((id, BTreeSet::new())));
        }
        let r = catch_unwind(AssertUnwindSafe(f));
        {
            let mut borrow = (*lock).borrow_mut();
            let (popped_id, touched_dependencies) =
                borrow.context_stack.pop().flatten().expect("unreachable");
            assert_eq!(popped_id, id);
            let notifications = borrow
                .stale_queue
                .update_dependency_set(id, touched_dependencies);
            let _ = ASignalRuntime::notify_all(&lock, notifications, borrow);
        }
        r.unwrap_or_else(|p| resume_unwind(p))
    }

    fn set_subscription(&self, id: Self::Symbol, enabled: bool) -> bool {
        let lock = self.critical_mutex.lock();
        let mut borrow = (*lock).borrow_mut();
        let (result, notifications) = borrow.stale_queue.set_subscription(id, enabled);
        let _ = ASignalRuntime::notify_all(&lock, notifications, borrow);
        result
    }

    fn update_or_enqueue(&self, id: Self::Symbol, f: impl 'static + Send + FnOnce()) {
        match self.critical_mutex.try_lock() {
            Some(lock) if !(*lock).borrow().callbacks.contains_key(&id) => panic!("Tried to update without starting the `pollinate::source::Source` first! (This panic may be sporadic when threading.)"),
            Some(lock)
                if (*lock).borrow().context_stack.is_empty()
                    && (*lock).borrow().stale_queue.peek().is_none() =>
            {
                f();
                self.propagate_from(id);
                return;
            }
            _ => (),
        }

        let mut update_queue = self.update_queue.lock().expect("unreachable");
        update_queue.push_back((id, Box::new(f)));
        drop(self.process_updates_if_ready(update_queue));
    }

    fn update_or_enqueue_blocking(&self, id: Self::Symbol, f: impl FnOnce()) {
        // This is indirected because the nested function's text size may be relatively large.
        //BLOCKED: Avoid the heap allocation once the `Allocator` API is stabilised.
        fn update_or_enqueue_blocking(
            this: &ASignalRuntime,
            id: ASymbol,
            f: Box<dyn '_ + FnOnce()>,
        ) {
            //FIXME: This could avoid the heap allocation.
            let barrier = Arc::new(Barrier::new(2));

            //FIXME: This can *maybe* may deadlock if called on the same thread while processing updates.
            //FIXME: This should NOT have to spawn new threads.
            thread::scope(|s| {
                s.spawn({
                    let barrier = Arc::clone(&barrier);
                    move || {
                        this.update_or_enqueue(id, move || {
                            barrier.wait();
                            barrier.wait();
                        })
                    }
                });
                barrier.wait();
                f();
                barrier.wait();
            });
            // Ensure propagation is complete:
            thread::scope(|s| {
                s.spawn({
                    let barrier = Arc::clone(&barrier);
                    move || {
                        this.update_or_enqueue(id, move || {
                            barrier.wait();
                        })
                    }
                });
                barrier.wait();
            })
        }
        update_or_enqueue_blocking(self, id, Box::new(f))
    }

    fn propagate_from(&self, id: Self::Symbol) {
        let lock = self.critical_mutex.lock();
        if (*lock)
            .borrow_mut()
            .stale_queue
            .mark_dependents_as_stale(id)
        {
            while let Some(current) = (|| (*lock).borrow_mut().stale_queue.next())() {
                let mut borrow = (*lock).borrow_mut();
                let &(callback_table, data) = borrow.callbacks.get(&current).expect("unreachable");
                if let &CallbackTable {
                    update: Some(update),
                    ..
                } = unsafe { &*callback_table }
                {
                    borrow.context_stack.push(Some((current, BTreeSet::new())));
                    drop(borrow);
                    let update = catch_unwind(|| unsafe { update(data) });
                    let mut borrow = (*lock).borrow_mut();
                    let (popped_id, touched_dependencies) =
                        borrow.context_stack.pop().flatten().expect("unreachable");
                    assert_eq!(popped_id, current);
                    let notifications = borrow
                        .stale_queue
                        .update_dependency_set(current, touched_dependencies);
                    borrow = ASignalRuntime::notify_all(&lock, notifications, borrow);
                    match update {
                        Ok(Update::Propagate) => {
                            let _ = borrow.stale_queue.mark_dependents_as_stale(current);
                        }
                        Ok(Update::Halt) => (),
                        Err(payload) => resume_unwind(payload),
                    }
                } else {
                    // As documented on `Callbacks`.
                    let _ = borrow.stale_queue.mark_dependents_as_stale(current);
                }
            }
        }
    }

    fn run_detached<T>(&self, f: impl FnOnce() -> T) -> T {
        let lock = self.critical_mutex.lock();
        let mut borrow = (*lock).borrow_mut();
        //TODO: Dirty queue isolation!
        borrow.context_stack.push(None);
        drop(borrow);
        let r = catch_unwind(AssertUnwindSafe(f));
        let mut borrow = (*lock).borrow_mut();
        assert_eq!(borrow.context_stack.pop(), Some(None));
        drop(borrow);
        r.unwrap_or_else(|payload| resume_unwind(payload))
    }

    fn refresh(&self, id: Self::Symbol) {
        let lock = self.critical_mutex.lock();
        let mut borrow = (*lock).borrow_mut();
        let is_stale = borrow.stale_queue.remove_stale(id);
        if is_stale {
            let &(callback_table, data) = borrow.callbacks.get(&id).expect("unreachable");
            if let &CallbackTable {
                update: Some(update),
                ..
            } = unsafe { &*callback_table }
            {
                borrow.context_stack.push(Some((id, BTreeSet::new())));
                drop(borrow);
                let r = catch_unwind(|| unsafe { update(data) });
                let mut borrow = (*lock).borrow_mut();
                let (popped_id, touched_dependencies) =
                    borrow.context_stack.pop().flatten().expect("unreachable");
                assert_eq!(popped_id, id);
                if let Err(payload) = r {
                    resume_unwind(payload)
                }

                let notifications = borrow
                    .stale_queue
                    .update_dependency_set(id, touched_dependencies);
                let _ = ASignalRuntime::notify_all(&lock, notifications, borrow);
            }
        }
    }

    fn stop(&self, id: Self::Symbol) {
        let lock = self.critical_mutex.lock();
        let mut borrow = (*lock).borrow_mut();
        if borrow
            .context_stack
            .iter()
            .filter_map(|s| s.as_ref())
            .any(|(symbol, _)| *symbol == id)
        {
            //TODO: Does this need to abort the process?
            panic!("Can't stop symbol while it is executing on the same thread.");
        }
        if borrow.stale_queue.is_subscribed(id) {
            let &(callback_table, data) = borrow
                .callbacks
                .get(&id)
                .expect("Tried to stop callbacks for a symbol that wasn't started.");
            {
                if let &CallbackTable {
                    on_subscribed_change: Some(on_subscribed_change),
                    ..
                } = unsafe { &*callback_table }
                {
                    unsafe { on_subscribed_change(data, false) }
                }
            }
        }
        let notifications = borrow.stale_queue.purge_id(id);
        let mut borrow = ASignalRuntime::notify_all(&lock, notifications, borrow);
        borrow.callbacks.remove(&id);
        drop(borrow);

        let mut update_queue = self.update_queue.lock().expect("infallible");
        update_queue.retain(|(item_id, _)| *item_id != id);

        // The order is important!
        drop(lock);
        drop(self.process_updates_if_ready(update_queue));
    }
}

static GLOBAL_SIGNAL_RUNTIME: ASignalRuntime = ASignalRuntime::new();

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlobalSignalRuntime;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GSRSymbol(ASymbol);

impl SignalRuntimeRef for GlobalSignalRuntime {
    type Symbol = GSRSymbol;

    fn next_id(&self) -> GSRSymbol {
        GSRSymbol((&GLOBAL_SIGNAL_RUNTIME).next_id())
    }

    fn reentrant_critical<T>(&self, f: impl FnOnce() -> T) -> T {
        (&GLOBAL_SIGNAL_RUNTIME).reentrant_critical(f)
    }

    fn touch(&self, id: Self::Symbol) {
        (&GLOBAL_SIGNAL_RUNTIME).touch(id.0)
    }

    unsafe fn start<T, D: ?Sized>(
        &self,
        id: Self::Symbol,
        f: impl FnOnce() -> T,
        callback_table: *const CallbackTable<D>,
        callback_data: *const D,
    ) -> T {
        (&GLOBAL_SIGNAL_RUNTIME).start(id.0, f, callback_table, callback_data)
    }

    fn update_dependency_set<T>(&self, id: Self::Symbol, f: impl FnOnce() -> T) -> T {
        (&GLOBAL_SIGNAL_RUNTIME).update_dependency_set(id.0, f)
    }

    fn set_subscription(&self, id: Self::Symbol, enabled: bool) -> bool {
        (&GLOBAL_SIGNAL_RUNTIME).set_subscription(id.0, enabled)
    }

    fn update_or_enqueue(&self, id: Self::Symbol, f: impl 'static + Send + FnOnce()) {
        (&GLOBAL_SIGNAL_RUNTIME).update_or_enqueue(id.0, f)
    }

    fn update_or_enqueue_blocking(&self, id: Self::Symbol, f: impl FnOnce()) {
        (&GLOBAL_SIGNAL_RUNTIME).update_or_enqueue_blocking(id.0, f)
    }

    fn propagate_from(&self, id: Self::Symbol) {
        (&GLOBAL_SIGNAL_RUNTIME).propagate_from(id.0)
    }

    fn run_detached<T>(&self, f: impl FnOnce() -> T) -> T {
        (&GLOBAL_SIGNAL_RUNTIME).run_detached(f)
    }

    fn refresh(&self, id: Self::Symbol) {
        (&GLOBAL_SIGNAL_RUNTIME).refresh(id.0)
    }

    fn stop(&self, id: Self::Symbol) {
        (&GLOBAL_SIGNAL_RUNTIME).stop(id.0)
    }
}

#[repr(C)]
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CallbackTable<T: ?Sized> {
    pub update: Option<unsafe extern "C" fn(*const T) -> Update>,
    pub on_subscribed_change: Option<unsafe extern "C" fn(*const T, subscribed: bool)>,
}

impl<T: ?Sized> CallbackTable<T> {
    pub fn into_erased_ptr(this: *const Self) -> *const CallbackTable<()> {
        unsafe { mem::transmute(this) }
    }

    pub fn into_erased(self) -> CallbackTable<()> {
        unsafe { mem::transmute(self) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Update {
    Propagate,
    Halt,
}
