use std::{borrow::Borrow, pin::Pin, sync::Mutex};

use pin_project::pin_project;
use pollinate::{
    runtime::{GlobalSignalRuntime, SignalRuntimeRef},
    slot::{Slot, Token},
    source::{NoCallbacks, Source},
};

use crate::traits::Subscribable;

#[pin_project]
#[must_use = "Signals do nothing unless they are polled or subscribed to."]
pub(crate) struct RawComputedUncachedMut<
    T: Send,
    F: Send + FnMut() -> T,
    SR: SignalRuntimeRef = GlobalSignalRuntime,
>(#[pin] Source<ForceSyncUnpin<Mutex<F>>, (), SR>);

#[pin_project]
struct ForceSyncUnpin<T: ?Sized>(#[pin] T);
unsafe impl<T: ?Sized> Sync for ForceSyncUnpin<T> {}

/// TODO: Safety documentation.
unsafe impl<T: Send, F: Send + FnMut() -> T, SR: SignalRuntimeRef + Sync> Sync
    for RawComputedUncachedMut<T, F, SR>
{
}

impl<T: Send, F: Send + FnMut() -> T, SR: SignalRuntimeRef> RawComputedUncachedMut<T, F, SR> {
    pub(crate) fn new(f: F, runtime: SR) -> Self {
        Self(Source::with_runtime(ForceSyncUnpin(f.into()), runtime))
    }

    fn get(self: Pin<&Self>) -> T {
        let mutex = self.touch();
        let mut f = mutex.lock().expect("unreachable");
        self.project_ref().0.update_dependency_set(move |_, _| f())
    }

    pub(crate) fn touch<'a>(self: Pin<&Self>) -> Pin<&Mutex<F>> {
        unsafe {
            self.project_ref()
                .0
                .project_or_init::<NoCallbacks>(|f, cache| Self::init(f, cache))
                .0
                .map_unchecked(|r| &r.0)
        }
    }

    pub fn pull<'a>(self: Pin<&'a Self>) -> impl 'a + Borrow<T> {
        let f = unsafe {
            self.project_ref()
                .0
                .pull_or_init::<NoCallbacks>(|f, cache| Self::init(f, cache))
                .0
                .map_unchecked(|r| &r.0)
        };
        self.project_ref()
            .0
            .update_dependency_set(move |_, _| f.lock().unwrap()())
    }
}

/// # Safety
///
/// These are the only functions that access `cache`.
/// Externally synchronised through guarantees on [`pollinate::init`].
impl<T: Send, F: Send + FnMut() -> T, SR: SignalRuntimeRef> RawComputedUncachedMut<T, F, SR> {
    unsafe fn init<'a>(_: Pin<&'a ForceSyncUnpin<Mutex<F>>>, lazy: Slot<'a, ()>) -> Token<'a> {
        lazy.write(())
    }
}

impl<T: Send, F: Send + FnMut() -> T, SR: SignalRuntimeRef> crate::Source<SR>
    for RawComputedUncachedMut<T, F, SR>
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
        Self::Value: Sync,
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

impl<T: Send, F: Send + FnMut() -> T, SR: SignalRuntimeRef> Subscribable<SR>
    for RawComputedUncachedMut<T, F, SR>
{
    fn pull<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Borrow<Self::Value>> {
        Box::new(self.pull())
    }

    fn unsubscribe(self: Pin<&Self>) -> bool {
        self.project_ref().0.unsubscribe()
    }
}
