use core::{
    fmt::Debug,
    num::NonZeroU64,
    sync::atomic::{AtomicU64, Ordering},
};
use std::{
    cell::RefCell,
    collections::{btree_map::Entry, BTreeMap, BTreeSet, VecDeque},
    mem,
    panic::{catch_unwind, resume_unwind, AssertUnwindSafe},
};

use stale_queue::StaleQueue;
use parking_lot::ReentrantMutex;

mod deferred_queue;
mod stale_queue;
mod work_queue;

pub trait SignalRuntimeRef: Clone {
    type Symbol: Clone + Copy;
    fn next_id(&self) -> Self::Symbol;
    fn reentrant_critical<T>(&self, f: impl FnOnce() -> T) -> T;
    fn touch(&self, id: Self::Symbol);
    unsafe fn start<T, D: ?Sized>(
        &self,
        id: Self::Symbol,
        f: impl FnOnce() -> T,
        callback: unsafe extern "C" fn(*const D),
        callback_data: *const D,
    ) -> T;
    /// # Returns
    ///
    /// Whether there was a change.
    fn set_subscription(&self, id: Self::Symbol, enabled: bool) -> bool;
    fn update_or_enqueue(&self, id: Self::Symbol, f: impl 'static + Send + FnOnce());
    fn propagate_from(&self, id: Self::Symbol);
    fn stop(&self, id: Self::Symbol);

    fn start_sensor<D: ?Sized>(
        &self,
        id: Self::Symbol,
        on_subscription_change: unsafe extern "C" fn(*const D, subscribed: bool),
        on_subscription_change_data: *const D,
    );
    fn stop_sensor(&self, id: Self::Symbol);
}

#[derive(Default)]
struct ASignalRuntime {
    source_counter: AtomicU64,
    critical_mutex: ReentrantMutex<RefCell<ASignalRuntime_>>,
}

unsafe impl Sync for ASignalRuntime {}

#[derive(Default)]
struct ASignalRuntime_ {
    stale_queue: StaleQueue<ASymbol>,
    stack: VecDeque<(ASymbol, BTreeSet<ASymbol>)>,
    callbacks: BTreeMap<ASymbol, (unsafe extern "C" fn(*const ()), *const ())>,
}

impl ASignalRuntime_ {
    const fn new() -> Self {
        Self {
            stale_queue: StaleQueue::new(),
            stack: VecDeque::new(),
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
        }
    }

    pub(crate) fn eval_dependents(&self, dependency: ASymbol) {
        let lock = self.critical_mutex.lock();
        (*lock)
            .borrow_mut()
            .stale_queue
            .mark_dependents_as_stale(dependency);
        while let Some(current) = (|| (*lock).borrow_mut().stale_queue.next())() {
            let mut borrow = (*lock).borrow_mut();
            if let Some(callback) = borrow.callbacks.get(&current).clone() {
                let (f, d) = callback.clone();
                borrow.stack.push_back((current, BTreeSet::new()));
                drop(borrow);
                let r = catch_unwind(|| unsafe { f(d) });
                let mut borrow = (*lock).borrow_mut();
                let (popped_id, touched_dependencies) =
                    borrow.stack.pop_back().expect("infallible");
                assert_eq!(popped_id, current);
                if let Err(payload) = r {
                    resume_unwind(payload)
                }
                borrow
                    .stale_queue
                    .update_dependencies(current, touched_dependencies);
                borrow.stale_queue.mark_dependents_as_stale(current);
            }
        }
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
        if let Some((context_id, touched)) = &mut borrow.stack.back_mut() {
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
        callback: unsafe extern "C" fn(*const D),
        callback_data: *const D,
    ) -> T {
        let lock = self.critical_mutex.lock();
        {
            let mut borrow = (*lock).borrow_mut();
            borrow.stale_queue.register_id(id);
            borrow.stack.push_back((id, BTreeSet::new()));
        }
        let r = catch_unwind(AssertUnwindSafe(f));
        {
            let mut borrow = (*lock).borrow_mut();
            let (popped_id, touched_dependencies) = borrow.stack.pop_back().expect("infallible");
            assert_eq!(popped_id, id);
            borrow
                .stale_queue
                .update_dependencies(id, touched_dependencies);
            match borrow.callbacks.entry(id) {
                Entry::Vacant(v) => {
                    v.insert((
                        unsafe {
                            //SAFETY: Due to `extern "C"`, these signatures are compatible.
                            mem::transmute::<
                                unsafe extern "C" fn(*const D),
                                unsafe extern "C" fn(*const ()),
                            >(callback)
                        },
                        callback_data.cast(),
                    ))
                }
                Entry::Occupied(_) => panic!("Can't call `start` again before calling `stop`."),
            };
        }
        r.unwrap_or_else(|p| resume_unwind(p))
    }

    fn set_subscription(&self, id: Self::Symbol, enabled: bool) -> bool {
        let lock = self.critical_mutex.lock();
        let mut borrow = (*lock).borrow_mut();
        borrow.stale_queue.set_subscription(id, enabled)
    }

    fn update_or_enqueue(&self, id: Self::Symbol, f: impl 'static + Send + FnOnce()) {
        match self.critical_mutex.try_lock() {
            Some(lock) if (*lock).borrow().stack.is_empty() => {
                f();
                self.propagate_from(id);
            }
            _ => todo!("update_or_enqueue: enqueue"),
        }
    }

    fn propagate_from(&self, id: Self::Symbol) {
        self.eval_dependents(id)
    }

    fn stop(&self, id: Self::Symbol) {
        let lock = self.critical_mutex.lock();
        let mut borrow = (*lock).borrow_mut();
        if borrow.stack.iter().any(|(symbol, _)| *symbol == id) {
            //TODO: Does this need to abort the process?
            panic!("Can't stop symbol while it is executing on the same thread.");
        }
        borrow.stale_queue.purge_id(id);
        borrow.callbacks.remove(&id);
    }

    fn start_sensor<D: ?Sized>(
        &self,
        id: Self::Symbol,
        on_subscription_change: unsafe extern "C" fn(*const D, subscribed: bool),
        on_subscription_change_data: *const D,
    ) {
        let lock = self.critical_mutex.lock();
        let mut borrow = (*lock).borrow_mut();

        borrow.stale_queue.start_sensor(
            id,
            unsafe {
                //SAFETY: Due to `extern "C"`, these signatures are compatible.
                mem::transmute::<
                    unsafe extern "C" fn(*const D, subscribed: bool),
                    unsafe extern "C" fn(*const (), subscribed: bool),
                >(on_subscription_change)
            },
            on_subscription_change_data.cast(),
        );
    }

    fn stop_sensor(&self, id: Self::Symbol) {
        let lock = self.critical_mutex.lock();
        let mut borrow = (*lock).borrow_mut();
        borrow.stale_queue.stop_sensor(id);
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
        callback: unsafe extern "C" fn(*const D),
        callback_data: *const D,
    ) -> T {
        (&GLOBAL_SIGNAL_RUNTIME).start(id.0, f, callback, callback_data)
    }

    fn set_subscription(&self, id: Self::Symbol, enabled: bool) -> bool {
        (&GLOBAL_SIGNAL_RUNTIME).set_subscription(id.0, enabled)
    }

    fn update_or_enqueue(&self, id: Self::Symbol, f: impl 'static + Send + FnOnce()) {
        (&GLOBAL_SIGNAL_RUNTIME).update_or_enqueue(id.0, f)
    }

    fn propagate_from(&self, id: Self::Symbol) {
        (&GLOBAL_SIGNAL_RUNTIME).propagate_from(id.0)
    }

    fn stop(&self, id: Self::Symbol) {
        (&GLOBAL_SIGNAL_RUNTIME).stop(id.0)
    }

    fn start_sensor<D: ?Sized>(
        &self,
        id: Self::Symbol,
        on_subscription_change: unsafe extern "C" fn(*const D, subscribed: bool),
        on_subscription_change_data: *const D,
    ) {
        (&GLOBAL_SIGNAL_RUNTIME).start_sensor(
            id.0,
            on_subscription_change,
            on_subscription_change_data,
        )
    }

    fn stop_sensor(&self, id: Self::Symbol) {
        (&GLOBAL_SIGNAL_RUNTIME).stop_sensor(id.0)
    }
}
