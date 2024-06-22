use std::pin::Pin;

use flourish::{
    raw::RawFold, AsSource, Fold, GlobalSignalRuntime, SignalRuntimeRef, Source, Update,
};

//TODO: Hide that `debounce` returns `Fold`?

pub fn debounce<'a, T: Send + Sync + Copy + PartialEq>(
    source: impl AsSource<'a, Source: Source<Value = T>> + Send,
) -> Fold<T> {
    debounce_with_runtime(source, GlobalSignalRuntime)
}

pub fn debounce_with_runtime<'a, T: Send + Sync + Copy + PartialEq, SR: SignalRuntimeRef>(
    source: impl AsSource<'a, Source: Source<Value = T>> + Send,
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

pub fn raw_debounce<'a, T: 'a + Send + Sync + Copy + PartialEq>(
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

pub fn pipe<P: IntoPipe>(pipe: P) -> P::Pipe {
    pipe.into_pipe()
}

pub trait IntoPipe: Sized {
    type Pipe;
    fn into_pipe(self) -> Self::Pipe;
}

impl<T0> IntoPipe for (T0,) {
    type Pipe = T0;

    fn into_pipe(self) -> Self::Pipe {
        self.0
    }
}

impl<T0, T1, F1: FnOnce(T0) -> T1> IntoPipe for (T0, F1) {
    type Pipe = T1;

    fn into_pipe(self) -> Self::Pipe {
        self.1(self.0)
    }
}

impl<T0, T1, T2, F1: FnOnce(T0) -> T1, F2: FnOnce(T1) -> T2> IntoPipe for (T0, F1, F2) {
    type Pipe = T2;

    fn into_pipe(self) -> Self::Pipe {
        self.2(self.1(self.0))
    }
}

impl<T0, T1, T2, T3, F1: FnOnce(T0) -> T1, F2: FnOnce(T1) -> T2, F3: FnOnce(T2) -> T3> IntoPipe
    for (T0, F1, F2, F3)
{
    type Pipe = T3;

    fn into_pipe(self) -> Self::Pipe {
        self.3(self.2(self.1(self.0)))
    }
}

impl<
        T0,
        T1,
        T2,
        T3,
        T4,
        F1: FnOnce(T0) -> T1,
        F2: FnOnce(T1) -> T2,
        F3: FnOnce(T2) -> T3,
        F4: FnOnce(T3) -> T4,
    > IntoPipe for (T0, F1, F2, F3, F4)
{
    type Pipe = T4;

    fn into_pipe(self) -> Self::Pipe {
        self.4(self.3(self.2(self.1(self.0))))
    }
}

impl<
        T0,
        T1,
        T2,
        T3,
        T4,
        T5,
        F1: FnOnce(T0) -> T1,
        F2: FnOnce(T1) -> T2,
        F3: FnOnce(T2) -> T3,
        F4: FnOnce(T3) -> T4,
        F5: FnOnce(T4) -> T5,
    > IntoPipe for (T0, F1, F2, F3, F4, F5)
{
    type Pipe = T5;

    fn into_pipe(self) -> Self::Pipe {
        self.5(self.4(self.3(self.2(self.1(self.0)))))
    }
}
