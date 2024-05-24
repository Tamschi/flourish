use core::{
    fmt::Debug,
    num::NonZeroU64,
    sync::atomic::{AtomicU64, Ordering},
};

mod deferred_queue;
mod dirty_queue;
mod work_queue;

pub trait SignalRuntimeRef: Clone {
    type Symbol: Clone + Copy;
    fn next_id(&self) -> Self::Symbol;
    fn stop(&self, id: Self::Symbol);
}

struct ASignalRuntime {
    source_counter: AtomicU64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct ASymbol(NonZeroU64);

impl ASignalRuntime {
    const fn new() -> Self {
        Self {
            source_counter: AtomicU64::new(0),
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

    fn stop(&self, id: Self::Symbol) {
        (&GLOBAL_SIGNAL_RUNTIME).stop(id.0)
    }
}
