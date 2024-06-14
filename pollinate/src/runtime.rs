use core::{
    fmt::Debug,
    num::NonZeroU64,
    sync::atomic::{AtomicU64, Ordering},
};
use std::{
    borrow::BorrowMut,
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    mem,
    panic::{catch_unwind, resume_unwind},
};

use dirty_queue::DirtyQueue;
use parking_lot::ReentrantMutex;

mod deferred_queue;
mod dirty_queue;
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
    dirty_queue: DirtyQueue<ASymbol>,
    stack: VecDeque<ASymbol>,
    callbacks: BTreeMap<ASymbol, (unsafe extern "C" fn(*const ()), *const ())>,
}

impl ASignalRuntime_ {
    const fn new() -> Self {
        Self {
            dirty_queue: DirtyQueue::new(),
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
        let mut borrow = Some((*lock).borrow_mut());
        while let Some(next) = borrow.as_mut().expect("unreachable").dirty_queue.next() {
            if let Some(callback) = borrow
                .as_mut()
                .expect("infallible")
                .callbacks
                .get(&next)
                .clone()
            {
                let (f, d) = callback.clone();
                borrow.as_mut().expect("unreachable").stack.push_back(next);
                borrow.take();
                let r = catch_unwind(|| unsafe { f(d) });
                borrow = Some((*lock).borrow_mut());
                assert_eq!(
                    borrow.as_mut().expect("unreachable").stack.pop_back(),
                    Some(dependency)
                );
                if let Err(payload) = r {
                    resume_unwind(payload)
                }
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
        //TODO
    }

    unsafe fn start<T, D: ?Sized>(
        &self,
        id: Self::Symbol,
        f: impl FnOnce() -> T,
        callback: unsafe extern "C" fn(*const D),
        callback_data: *const D,
    ) -> T {
        f()
        //TODO
    }

    fn set_subscription(&self, id: Self::Symbol, enabled: bool) -> bool {
        let lock = self.critical_mutex.lock();
        let mut borrow = (*lock).borrow_mut();
        borrow.dirty_queue.set_subscription(id, enabled)
    }

    fn update_or_enqueue(&self, id: Self::Symbol, f: impl 'static + Send + FnOnce()) {
        //TODO
        self.propagate_from(id);
    }

    fn propagate_from(&self, id: Self::Symbol) {
        self.eval_dependents(id)
    }

    fn stop(&self, id: Self::Symbol) {
        let lock = self.critical_mutex.lock();
        let mut borrow = (*lock).borrow_mut();
        if borrow.stack.contains(&id) {
            //TODO: Does this need to abort the process?
            panic!("Can't stop symbol while it is executing on the same thread.");
        }
        borrow.dirty_queue.purge_id(id);
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

        borrow.dirty_queue.start_sensor(
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
        borrow.dirty_queue.stop_sensor(id);
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
