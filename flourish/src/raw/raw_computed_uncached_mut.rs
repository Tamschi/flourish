use std::{
    borrow::Borrow,
    ops::Deref,
    pin::Pin,
    sync::{Mutex, RwLockReadGuard},
};

use pin_project::pin_project;
use pollinate::{
    runtime::{GlobalSignalRuntime, SignalRuntimeRef, Update},
    slot::{Slot, Token},
    source::{Callbacks, Source},
};

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

pub(crate) struct RawComputedUncachedMutGuard<'a, T: ?Sized>(RwLockReadGuard<'a, T>);

impl<'a, T: ?Sized> Deref for RawComputedUncachedMutGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.borrow()
    }
}

impl<'a, T: ?Sized> Borrow<T> for RawComputedUncachedMutGuard<'a, T> {
    fn borrow(&self) -> &T {
        self.0.borrow()
    }
}

/// TODO: Safety documentation.
unsafe impl<T: Send, F: Send + FnMut() -> T, SR: SignalRuntimeRef + Sync> Sync
    for RawComputedUncachedMut<T, F, SR>
{
}

impl<T: Send, F: Send + FnMut() -> T, SR: SignalRuntimeRef> RawComputedUncachedMut<T, F, SR> {
    pub(crate) fn new(f: F, runtime: SR) -> Self {
        Self(Source::with_runtime(ForceSyncUnpin(f.into()), runtime))
    }

    //TODO: This doesn't track right.
    fn get(self: Pin<&Self>) -> T {
        self.touch().lock().expect("unreachable")()
    }

    pub(crate) fn touch<'a>(self: Pin<&Self>) -> Pin<&Mutex<F>> {
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
impl<T: Send, F: Send + FnMut() -> T, SR: SignalRuntimeRef>
    Callbacks<ForceSyncUnpin<Mutex<F>>, (), SR> for E
{
    const UPDATE: Option<
        unsafe fn(eager: Pin<&ForceSyncUnpin<Mutex<F>>>, lazy: Pin<&()>) -> Update,
    > = {
        unsafe fn eval<T: Send, F: Send + FnMut() -> T>(
            _: Pin<&ForceSyncUnpin<Mutex<F>>>,
            _: Pin<&()>,
        ) -> Update {
            Update::Propagate
        }
        Some(eval)
    };

    const ON_SUBSCRIBED_CHANGE: Option<
        unsafe fn(eager: Pin<&ForceSyncUnpin<Mutex<F>>>, lazy: Pin<&()>, subscribed: bool),
    > = None;
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
        Self::Value: 'a + Sync,
    {
        Box::new(self.get())
    }

    fn clone_runtime_ref(&self) -> SR
    where
        SR: Sized,
    {
        self.0.clone_runtime_ref()
    }
}
