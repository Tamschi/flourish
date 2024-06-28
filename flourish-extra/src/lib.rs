use std::{
    ops::{AddAssign, Sub},
    pin::Pin,
};

use flourish::{
    raw::{computed, folded, merged},
    SignalRuntimeRef, Source, SubscriptionSR, Update,
};
use num_traits::Zero;

//BLOCKED: `merge`, `filter` and `fold` (as curried operators) wait on <https://github.com/rust-lang/rust/issues/99697>.

pub fn debounce<'a, T: 'a + Send + Sync + Copy + PartialEq, SR: 'a + SignalRuntimeRef>(
    source: impl 'a + Source<SR, Value = T>,
) -> impl 'a + Source<SR, Value = T> {
    merged(source, |current, next| {
        if current != &next {
            *current = next;
            Update::Propagate
        } else {
            Update::Halt
        }
    })
}

pub fn delta<
    'a,
    T: 'a + Send + Sync + Copy + Sub<Output: Zero + Send + Sync + Copy>,
    SR: 'a + SignalRuntimeRef,
>(
    source: impl 'a + Source<SR, Value = T>,
) -> impl 'a + Source<SR, Value = T::Output> {
    let mut previous = None;
    folded(source, T::Output::zero(), move |delta, next| {
        if let Some(previous) = previous {
            *delta = next - previous;
        }
        previous = Some(next);
        Update::Propagate
    })
}

pub fn sparse_tally<
    'a,
    Tally: 'a + Zero + Send + Sync + Copy + AddAssign<T>,
    T: 'a + Send + Sync + Copy,
    SR: 'a + SignalRuntimeRef,
>(
    source: impl 'a + Source<SR, Value = T>,
) -> impl 'a + Source<SR, Value = Tally> {
    folded(source, Tally::zero(), |tally, next| {
        *tally += next;
        Update::Propagate
    })
}

pub fn eager_tally<
    'a,
    Tally: Zero + Send + Sync + Copy + AddAssign<T>,
    T: 'a + Send + Sync + Copy,
    SR: 'a + SignalRuntimeRef,
>(
    source: impl 'a + Source<SR, Value = T>,
) -> SubscriptionSR<'a, Tally, SR> {
    SubscriptionSR::new(sparse_tally(source))
}

pub fn filtered<'a, T: 'a + Send + Sync + Copy, SR: 'a + SignalRuntimeRef>(
    source: impl 'a + Source<SR, Value = T>,
    mut f: impl 'a + Send + FnMut(&T) -> bool,
) -> impl 'a + Source<SR, Value = Option<T>> {
    folded(source, None, move |current, next| {
        if f(&next) {
            *current = Some(next);
            Update::Propagate
        } else {
            Update::Halt
        }
    })
}

pub fn mapped<'a, T: 'a + Send + Sync + Copy, U: 'a + Send + Clone, SR: 'a + SignalRuntimeRef>(
    source: impl 'a + Source<SR, Value = T>,
    mut f: impl 'a + Send + FnMut(T) -> U,
) -> impl 'a + Source<SR, Value = U> {
    let runtime = source.clone_runtime_ref();
    computed(
        move || f(unsafe { Pin::new_unchecked(&source) }.get()),
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
