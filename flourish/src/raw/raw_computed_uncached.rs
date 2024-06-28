use std::{borrow::Borrow, pin::Pin};

use pin_project::pin_project;
use pollinate::{
    runtime::{GlobalSignalRuntime, SignalRuntimeRef, Update},
    slot::{Slot, Token},
    source::{Callbacks, Source},
};

#[pin_project]
#[must_use = "Signals do nothing unless they are polled or subscribed to."]
pub(crate) struct RawComputedUncached<
    T: Send,
    F: Send + Sync + Fn() -> T,
    SR: SignalRuntimeRef = GlobalSignalRuntime,
>(#[pin] Source<ForceSyncUnpin<F>, (), SR>);

#[pin_project]
struct ForceSyncUnpin<T: ?Sized>(#[pin] T);
unsafe impl<T: ?Sized> Sync for ForceSyncUnpin<T> {}

/// TODO: Safety documentation.
unsafe impl<T: Send, F: Send + Sync + Fn() -> T, SR: SignalRuntimeRef + Sync> Sync
    for RawComputedUncached<T, F, SR>
{
}

impl<T: Send, F: Send + Sync + Fn() -> T, SR: SignalRuntimeRef> RawComputedUncached<T, F, SR> {
    pub(crate) fn new(f: F, runtime: SR) -> Self {
        Self(Source::with_runtime(ForceSyncUnpin(f.into()), runtime))
    }

    //TODO: This doesn't track right.
    fn get(self: Pin<&Self>) -> T {
        self.touch()()
    }

    pub(crate) fn touch<'a>(self: Pin<&Self>) -> Pin<&F> {
        unsafe {
            self.project_ref()
                .0
                .project_or_init::<E>(|f, cache| Self::init(f, cache))
                .0
                .map_unchecked(|r| &r.0)
        }
    }
}

enum E {}
impl<T: Send, F: Send + Sync + Fn() -> T, SR: SignalRuntimeRef> Callbacks<ForceSyncUnpin<F>, (), SR>
    for E
{
    const UPDATE: Option<unsafe fn(eager: Pin<&ForceSyncUnpin<F>>, lazy: Pin<&()>) -> Update> = {
        unsafe fn eval<T: Send, F: Send + Sync + Fn() -> T>(
            _: Pin<&ForceSyncUnpin<F>>,
            _: Pin<&()>,
        ) -> Update {
            Update::Propagate
        }
        Some(eval)
    };

    const ON_SUBSCRIBED_CHANGE: Option<
        unsafe fn(eager: Pin<&ForceSyncUnpin<F>>, lazy: Pin<&()>, subscribed: bool),
    > = None;
}

/// # Safety
///
/// These are the only functions that access `cache`.
/// Externally synchronised through guarantees on [`pollinate::init`].
impl<T: Send, F: Send + Sync + Fn() -> T, SR: SignalRuntimeRef> RawComputedUncached<T, F, SR> {
    unsafe fn init<'a>(_: Pin<&'a ForceSyncUnpin<F>>, lazy: Slot<'a, ()>) -> Token<'a> {
        lazy.write(())
    }
}

impl<T: Send, F: Send + Sync + Fn() -> T, SR: SignalRuntimeRef> crate::Source<SR>
    for RawComputedUncached<T, F, SR>
{
    type Value = T;

    fn touch(self: Pin<&Self>) {
        self.touch();
    }

    fn get(self: Pin<&Self>) -> Self::Value
    where
        Self::Value: Sync + Copy,
    {
        self.get()
    }

    fn get_clone(self: Pin<&Self>) -> Self::Value
    where
        Self::Value: Sync + Clone,
    {
        self.get()
    }

    fn get_exclusive(self: Pin<&Self>) -> Self::Value
    where
        Self::Value: Copy,
    {
        self.get()
    }

    fn get_clone_exclusive(self: Pin<&Self>) -> Self::Value
    where
        Self::Value: Clone,
    {
        self.get()
    }

    fn read<'a>(self: Pin<&'a Self>) -> Box<dyn 'a + Borrow<Self::Value>>
    where
        Self::Value: 'a + Sync,
    {
        Box::new(self.get())
    }

    fn read_exclusive<'a>(self: Pin<&'a Self>) -> Box<dyn 'a + Borrow<Self::Value>> {
        Box::new(self.get())
    }

    fn clone_runtime_ref(&self) -> SR
    where
        SR: Sized,
    {
        self.0.clone_runtime_ref()
    }
}
