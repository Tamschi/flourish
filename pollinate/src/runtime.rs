use std::{
    num::NonZeroU64,
    sync::atomic::{AtomicU64, Ordering},
};

pub trait SignalRuntimeRef: Clone {
    fn next_source_id_number(&self) -> NonZeroU64;
}

struct ASignalRuntime {
    source_counter: AtomicU64,
}

impl ASignalRuntime {
    const fn new() -> Self {
        Self {
            source_counter: AtomicU64::new(0),
        }
    }
}

impl SignalRuntimeRef for &ASignalRuntime {
    fn next_source_id_number(&self) -> NonZeroU64 {
        (self.source_counter.fetch_add(1, Ordering::SeqCst) + 1)
            .try_into()
            .expect("infallible within reasonable time")
    }
}

static GLOBAL_SIGNAL_RUNTIME: ASignalRuntime = ASignalRuntime::new();

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlobalSignalRuntime;

impl SignalRuntimeRef for GlobalSignalRuntime {
    fn next_source_id_number(&self) -> NonZeroU64 {
        (&GLOBAL_SIGNAL_RUNTIME).next_source_id_number()
    }
}
