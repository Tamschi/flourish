use core::{
    fmt::Debug,
    num::NonZeroU64,
    sync::atomic::{AtomicU64, Ordering},
};

use parking_lot::ReentrantMutex;

mod deferred_queue;
mod dirty_queue;
mod work_queue;

pub trait SignalRuntimeRef: Clone {
    type Symbol: Clone + Copy;
    fn next_id(&self) -> Self::Symbol;
    fn reentrant_critical<T>(&self, f: impl FnOnce() -> T) -> T;
    fn touch(&self, id: Self::Symbol);
    fn start<T, D: ?Sized>(
        &self,
        id: Self::Symbol,
        f: impl FnOnce() -> T,
        callback: unsafe extern "C" fn(*const D),
        callback_data: *const D,
    ) -> T;
    fn set_subscription(&self, id: Self::Symbol, enabled: bool);
    fn update_or_enqueue(&self, id: Self::Symbol, f: impl 'static + Send + FnOnce());
    fn propagate_from(&self, id: Self::Symbol);
    fn stop(&self, id: Self::Symbol);
}

struct ASignalRuntime {
    source_counter: AtomicU64,
    critical_mutex: ReentrantMutex<()>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct ASymbol(NonZeroU64);

impl ASignalRuntime {
    const fn new() -> Self {
        Self {
            source_counter: AtomicU64::new(0),
            critical_mutex: ReentrantMutex::new(()),
        }
    }
}

impl SignalRuntimeRef for &ASignalRuntime {
    type Symbol = ASymbol;

    fn next_id(&self) -> Self::Symbol {
        ASymbol(
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

    fn start<T, D: ?Sized>(
        &self,
        id: Self::Symbol,
        f: impl FnOnce() -> T,
        callback: unsafe extern "C" fn(*const D),
        callback_data: *const D,
    ) -> T {
        f()
        //TODO
    }

    fn set_subscription(&self, id: Self::Symbol, enabled: bool) {
        //TODO
    }

    fn update_or_enqueue(&self, id: Self::Symbol, f: impl 'static + Send + FnOnce()) {
        //TODO
    }

    fn propagate_from(&self, id: Self::Symbol) {
        //TODO
    }

    fn stop(&self, id: Self::Symbol) {
        //TODO
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

    fn start<T, D: ?Sized>(
        &self,
        id: Self::Symbol,
        f: impl FnOnce() -> T,
        callback: unsafe extern "C" fn(*const D),
        callback_data: *const D,
    ) -> T {
        (&GLOBAL_SIGNAL_RUNTIME).start(id.0, f, callback, callback_data)
    }

    fn set_subscription(&self, id: Self::Symbol, enabled: bool) {
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
}
