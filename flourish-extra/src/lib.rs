use std::pin::Pin;

use flourish::{
    raw::RawFold, AsSource, Fold, GlobalSignalRuntime, SignalRuntimeRef, Source, Update,
};

pub fn debounce<T: Send + Sync + Copy + PartialEq>(
    source: impl for<'a> AsSource<'a, Source: Source<Value = T>> + Send,
) -> Fold<T> {
    debounce_with_runtime(source, GlobalSignalRuntime)
}

pub fn debounce_with_runtime<T: Send + Sync + Copy + PartialEq, SR: SignalRuntimeRef>(
    source: impl for<'a> AsSource<'a, Source: Source<Value = T>> + Send,
    runtime: SR,
) -> Fold<T, SR> {
    Fold::with_runtime(
        move || {
            unsafe { Pin::new_unchecked(&source) }
                .as_ref()
                .as_source()
                .get()
        },
        |current, next| {
            if current != &next {
                *current = next;
                Update::Propagate
            } else {
                Update::Halt
            }
        },
        runtime,
    )
}

pub fn raw_debounce<
    'a,
    T: 'a + Send + Sync + Copy + PartialEq,
    SR: 'a + Send + SignalRuntimeRef<Symbol: Send>,
>(
    source: impl 'a + AsSource<'a, Source: Source<Value = T>> + Send,
) -> impl AsSource<'a, Source: Source<Value = T>> + Send {
    raw_debounce_with_runtime(source, GlobalSignalRuntime)
}

pub fn raw_debounce_with_runtime<
    'a,
    T: 'a + Send + Sync + Copy + PartialEq,
    SR: 'a + Send + SignalRuntimeRef<Symbol: Send>,
>(
    source: impl 'a + AsSource<'a, Source: Source<Value = T>> + Send,
    runtime: SR,
) -> impl AsSource<'a, Source: Source<Value = T>> + Send {
    RawFold::with_runtime(
        move || {
            unsafe { Pin::new_unchecked(&source) }
                .as_ref()
                .as_source()
                .get()
        },
        |current, next| {
            if current != &next {
                *current = next;
                Update::Propagate
            } else {
                Update::Halt
            }
        },
        runtime,
    )
}
