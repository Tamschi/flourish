use std::{
    ops::{AddAssign, Sub},
    pin::Pin,
};

use flourish::{
    raw::{computed, folded, merged},
    SignalRuntimeRef, Source, Subscribable, SubscriptionSR, Update,
};
use num_traits::Zero;

pub mod future;

//BLOCKED: `merge`, `filter` and `fold` (as curried operators) wait on <https://github.com/rust-lang/rust/issues/99697>.

//TODO: These have extraneous bounds. Change to accept closures to remove some `T: Sync + Copy` bounds.

pub fn debounce<'a, T: 'a + Send + PartialEq, SR: 'a + SignalRuntimeRef>(
    fn_pin: impl 'a + Send + FnMut() -> T,
    runtime: SR,
) -> impl 'a + Subscribable<SR, Value = T> {
    merged(
        fn_pin,
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

pub fn debounce_from_source<
    'a,
    T: 'a + Send + Sync + Copy + PartialEq,
    SR: 'a + SignalRuntimeRef,
>(
    source: impl 'a + Source<SR, Value = T>,
) -> impl 'a + Subscribable<SR, Value = T> {
    let runtime = source.clone_runtime_ref();
    debounce(
        move || unsafe { Pin::new_unchecked(&source) }.get(),
        runtime,
    )
}

pub fn delta<'a, V: 'a + Send, T: 'a + Send + Zero, SR: 'a + SignalRuntimeRef>(
    mut fn_pin: impl 'a + Send + FnMut() -> V,
    runtime: SR,
) -> impl 'a + Subscribable<SR, Value = T>
where
    for<'b> &'b V: Sub<Output = T>,
{
    let mut previous = None;
    folded(
        <&'a V as Sub>::Output::zero(),
        move |delta| {
            let next: V = fn_pin();
            if let Some(previous) = previous.as_mut() {
                *delta = &next - &*previous;
            }
            previous = Some(next);
            Update::Propagate
        },
        runtime,
    )
}

pub fn delta_from_source<
    'a,
    V: 'a + Send + Sync + Copy,
    T: 'a + Send + Zero,
    SR: 'a + SignalRuntimeRef,
>(
    source: impl 'a + Source<SR, Value = V>,
) -> impl 'a + Subscribable<SR, Value = T>
where
    for<'b> &'b V: Sub<Output = T>,
{
    let runtime = source.clone_runtime_ref();
    delta(
        move || unsafe { Pin::new_unchecked(&source) }.get(),
        runtime,
    )
}

pub fn sparse_tally<'a, V: 'a, T: 'a + Zero + Send + AddAssign<V>, SR: 'a + SignalRuntimeRef>(
    mut fn_pin: impl 'a + Send + FnMut() -> V,
    runtime: SR,
) -> impl 'a + Subscribable<SR, Value = T> {
    folded(
        T::zero(),
        move |tally| {
            *tally += fn_pin();
            Update::Propagate
        },
        runtime,
    )
}

pub fn sparse_tally_from_source<
    'a,
    V: 'a + Sync + Copy,
    T: 'a + Zero + Send + Sync + Copy + AddAssign<V>,
    SR: 'a + SignalRuntimeRef,
>(
    source: impl 'a + Source<SR, Value = V>,
) -> impl 'a + Subscribable<SR, Value = T> {
    let runtime = source.clone_runtime_ref();
    sparse_tally(
        move || unsafe { Pin::new_unchecked(&source) }.get(),
        runtime,
    )
}

pub fn eager_tally<
    'a,
    V: 'a,
    T: 'a + Zero + Send + Clone + AddAssign<V>,
    SR: 'a + SignalRuntimeRef,
>(
    fn_pin: impl 'a + Send + FnMut() -> V,
    runtime: SR,
) -> SubscriptionSR<'a, T, SR> {
    SubscriptionSR::new(sparse_tally(fn_pin, runtime))
}

pub fn eager_tally_from_source<
    'a,
    V: 'a + Send + Sync + Copy,
    T: Zero + Send + Sync + Copy + AddAssign<V>,
    SR: 'a + SignalRuntimeRef,
>(
    source: impl 'a + Source<SR, Value = V>,
) -> SubscriptionSR<'a, T, SR> {
    SubscriptionSR::new(sparse_tally_from_source(source))
}

pub fn map<'a, T: 'a + Send + Sync + Copy, U: 'a + Send + Clone, SR: 'a + SignalRuntimeRef>(
    mut fn_pin: impl 'a + Send + FnMut() -> T,
    mut map_fn_pin: impl 'a + Send + FnMut(T) -> U,
    runtime: SR,
) -> impl 'a + Subscribable<SR, Value = U> {
    computed(move || map_fn_pin(fn_pin()), runtime)
}

pub fn map_from_source<
    'a,
    T: 'a + Send + Sync + Copy,
    U: 'a + Send + Clone,
    SR: 'a + SignalRuntimeRef,
>(
    source: impl 'a + Source<SR, Value = T>,
    map_fn_pin: impl 'a + Send + FnMut(T) -> U,
) -> impl 'a + Subscribable<SR, Value = U> {
    let runtime = source.clone_runtime_ref();
    map(
        move || unsafe { Pin::new_unchecked(&source) }.get(),
        map_fn_pin,
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
