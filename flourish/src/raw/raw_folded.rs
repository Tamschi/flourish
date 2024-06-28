use std::{
    borrow::{Borrow, BorrowMut},
    cell::UnsafeCell,
    mem::size_of,
    ops::Deref,
    pin::Pin,
    sync::{RwLock, RwLockReadGuard},
};

use pin_project::pin_project;
use pollinate::{
    runtime::{GlobalSignalRuntime, SignalRuntimeRef, Update},
    slot::{Slot, Token},
    source::{Callbacks, Source},
};

use crate::utils::conjure_zst;

#[pin_project]
#[must_use = "Signals do nothing unless they are polled or subscribed to."]
pub(crate) struct RawFolded<
    T: Send,
    F: Send + FnMut(&mut T) -> Update,
    SR: SignalRuntimeRef = GlobalSignalRuntime,
>(#[pin] Source<(ForceSyncUnpin<RwLock<T>>, ForceSyncUnpin<UnsafeCell<F>>), (), SR>);

#[pin_project]
struct ForceSyncUnpin<T: ?Sized>(T);
unsafe impl<T: ?Sized> Sync for ForceSyncUnpin<T> {}

pub(crate) struct RawFoldedGuard<'a, T: ?Sized>(RwLockReadGuard<'a, T>);

impl<'a, T: ?Sized> Deref for RawFoldedGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.borrow()
    }
}

impl<'a, T: ?Sized> Borrow<T> for RawFoldedGuard<'a, T> {
    fn borrow(&self) -> &T {
        self.0.borrow()
    }
}

/// TODO: Safety documentation.
unsafe impl<T: Send, F: Send + FnMut(&mut T) -> Update, SR: SignalRuntimeRef + Sync> Sync
    for RawFolded<T, F, SR>
{
}

/// See [rust-lang#98931](https://github.com/rust-lang/rust/issues/98931).
impl<T: Send, F: Send + FnMut(&mut T) -> Update, SR: SignalRuntimeRef> RawFolded<T, F, SR> {
    pub fn new(init: T, f: F, runtime: SR) -> Self {
        Self(Source::with_runtime(
            (ForceSyncUnpin(init.into()), ForceSyncUnpin(f.into())),
            runtime,
        ))
    }

    pub fn get(self: Pin<&Self>) -> T
    where
        T: Sync + Copy,
    {
        if size_of::<T>() == 0 {
            // The read is unobservable, so just skip locking.
            self.touch();
            conjure_zst()
        } else {
            *self.read()
        }
    }

    pub fn get_clone(self: Pin<&Self>) -> T
    where
        T: Sync + Clone,
    {
        self.read().clone()
    }

    pub fn read<'a>(self: Pin<&'a Self>) -> RawFoldedGuard<'a, T>
    where
        T: Sync,
    {
        RawFoldedGuard(self.touch().read().unwrap())
    }

    pub fn get_exclusive(self: Pin<&Self>) -> T
    where
        T: Copy,
    {
        if size_of::<T>() == 0 {
            // The read is unobservable, so just skip locking.
            self.touch();
            conjure_zst()
        } else {
            self.get_clone_exclusive()
        }
    }

    pub fn get_clone_exclusive(self: Pin<&Self>) -> T
    where
        T: Clone,
    {
        self.touch().write().unwrap().clone()
    }

    pub(crate) fn touch(self: Pin<&Self>) -> &RwLock<T> {
        unsafe {
            &Pin::into_inner_unchecked(
                self.project_ref()
                    .0
                    .project_or_init::<E>(|state, cache| Self::init(state, cache))
                    .0,
            )
            .0
             .0
        }
    }
}

enum E {}
impl<T: Send, F: Send + ?Sized + FnMut(&mut T) -> Update, SR: SignalRuntimeRef>
    Callbacks<(ForceSyncUnpin<RwLock<T>>, ForceSyncUnpin<UnsafeCell<F>>), (), SR> for E
{
    const UPDATE: Option<
        unsafe fn(
            eager: Pin<&(ForceSyncUnpin<RwLock<T>>, ForceSyncUnpin<UnsafeCell<F>>)>,
            lazy: Pin<&()>,
        ) -> Update,
    > = {
        unsafe fn eval<T: Send, F: Send + ?Sized + FnMut(&mut T) -> Update>(
            state: Pin<&(ForceSyncUnpin<RwLock<T>>, ForceSyncUnpin<UnsafeCell<F>>)>,
            _: Pin<&()>,
        ) -> Update {
            let f = &mut *state.1 .0.get();
            f(&mut *state.0 .0.write().unwrap())
        }
        Some(eval)
    };

    const ON_SUBSCRIBED_CHANGE: Option<
        unsafe fn(
            eager: Pin<&(ForceSyncUnpin<RwLock<T>>, ForceSyncUnpin<UnsafeCell<F>>)>,
            lazy: Pin<&()>,
            subscribed: bool,
        ),
    > = None;
}

/// # Safety
///
/// These are the only functions that access `cache`.
/// Externally synchronised through guarantees on [`pollinate::init`].
impl<T: Send, F: Send + FnMut(&mut T) -> Update, SR: SignalRuntimeRef> RawFolded<T, F, SR> {
    unsafe fn init<'a>(
        state: Pin<&'a (ForceSyncUnpin<RwLock<T>>, ForceSyncUnpin<UnsafeCell<F>>)>,
        lazy: Slot<'a, ()>,
    ) -> Token<'a> {
        let mut guard = state.0 .0.try_write().expect("unreachable");
        let _ = (&mut *state.1 .0.get())(guard.borrow_mut());
        lazy.write(())
    }
}

impl<T: Send, F: Send + FnMut(&mut T) -> Update, SR: SignalRuntimeRef> crate::Source<SR>
    for RawFolded<T, F, SR>
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
        self.get_clone()
    }

    fn get_exclusive(self: Pin<&Self>) -> Self::Value
    where
        Self::Value: Copy,
    {
        self.get_exclusive()
    }

    fn get_clone_exclusive(self: Pin<&Self>) -> Self::Value
    where
        Self::Value: Clone,
    {
        self.get_clone_exclusive()
    }

    fn read<'a>(self: Pin<&'a Self>) -> Box<dyn 'a + Borrow<Self::Value>>
    where
        Self::Value: 'a + Sync,
    {
        Box::new(self.read())
    }

    fn clone_runtime_ref(&self) -> SR
    where
        SR: Sized,
    {
        self.0.clone_runtime_ref()
    }
}
