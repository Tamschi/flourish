use std::{
    borrow::Borrow,
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
pub(crate) struct RawFold<
    T: Send,
    S: Send + FnMut() -> T,
    M: Send + FnMut(&mut T, T) -> Update,
    SR: SignalRuntimeRef = GlobalSignalRuntime,
>(#[pin] Source<ForceSyncUnpin<UnsafeCell<(S, M)>>, ForceSyncUnpin<RwLock<T>>, SR>);

#[pin_project]
struct ForceSyncUnpin<T: ?Sized>(T);
unsafe impl<T: ?Sized> Sync for ForceSyncUnpin<T> {}

pub(crate) struct RawFoldGuard<'a, T: ?Sized>(RwLockReadGuard<'a, T>);

impl<'a, T: ?Sized> Deref for RawFoldGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.borrow()
    }
}

impl<'a, T: ?Sized> Borrow<T> for RawFoldGuard<'a, T> {
    fn borrow(&self) -> &T {
        self.0.borrow()
    }
}

/// TODO: Safety documentation.
unsafe impl<
        T: Send,
        S: Send + FnMut() -> T,
        M: Send + FnMut(&mut T, T) -> Update,
        SR: SignalRuntimeRef + Sync,
    > Sync for RawFold<T, S, M, SR>
{
}

/// See [rust-lang#98931](https://github.com/rust-lang/rust/issues/98931).
impl<T: Send, S: Send + FnMut() -> T, M: Send + FnMut(&mut T, T) -> Update> RawFold<T, S, M> {
    pub fn new(select: S, merge: M) -> Self {
        Self::with_runtime(select, merge, GlobalSignalRuntime)
    }
}

impl<
        T: Send,
        S: Send + FnMut() -> T,
        M: Send + FnMut(&mut T, T) -> Update,
        SR: SignalRuntimeRef,
    > RawFold<T, S, M, SR>
{
    pub fn with_runtime(select: S, merge: M, runtime: SR) -> Self
    where
        M: Sized,
        SR: SignalRuntimeRef,
    {
        Self(Source::with_runtime(
            ForceSyncUnpin((select, merge).into()),
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

    pub fn read<'a>(self: Pin<&'a Self>) -> RawFoldGuard<'a, T>
    where
        T: Sync,
    {
        RawFoldGuard(self.touch().read().unwrap())
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
            self.project_ref()
                .0
                .project_or_init::<E>(|f, cache| Self::init(f, cache))
                .1
                .project_ref()
                .0
        }
    }

    pub(crate) fn pull(self: Pin<&Self>) -> &RwLock<T> {
        unsafe {
            self.project_ref()
                .0
                .pull_or_init::<E>(|f, cache| Self::init(f, cache))
                .1
                .project_ref()
                .0
        }
    }
}

enum E {}
impl<
        T: Send,
        S: Send + FnMut() -> T,
        M: Send + ?Sized + FnMut(&mut T, T) -> Update,
        SR: SignalRuntimeRef,
    > Callbacks<ForceSyncUnpin<UnsafeCell<(S, M)>>, ForceSyncUnpin<RwLock<T>>, SR> for E
{
    const UPDATE: Option<
        unsafe fn(
            eager: Pin<&ForceSyncUnpin<UnsafeCell<(S, M)>>>,
            lazy: Pin<&ForceSyncUnpin<RwLock<T>>>,
        ) -> Update,
    > = {
        unsafe fn eval<
            T: Send,
            S: Send + FnMut() -> T,
            M: Send + ?Sized + FnMut(&mut T, T) -> Update,
        >(
            f: Pin<&ForceSyncUnpin<UnsafeCell<(S, M)>>>,
            cache: Pin<&ForceSyncUnpin<RwLock<T>>>,
        ) -> Update {
            let (select, merge) = &mut *f.project_ref().0.get();
            // Avoid locking over `select()` here.
            let next_value = select();
            merge(&mut *cache.project_ref().0.write().unwrap(), next_value)
        }
        Some(eval)
    };

    const ON_SUBSCRIBED_CHANGE: Option<
        unsafe fn(
            eager: Pin<&ForceSyncUnpin<UnsafeCell<(S, M)>>>,
            lazy: Pin<&ForceSyncUnpin<RwLock<T>>>,
            subscribed: bool,
        ),
    > = None;
}

/// # Safety
///
/// These are the only functions that access `cache`.
/// Externally synchronised through guarantees on [`pollinate::init`].
impl<
        T: Send,
        S: Send + FnMut() -> T,
        M: Send + FnMut(&mut T, T) -> Update,
        SR: SignalRuntimeRef,
    > RawFold<T, S, M, SR>
{
    unsafe fn init<'a>(
        f: Pin<&'a ForceSyncUnpin<UnsafeCell<(S, M)>>>,
        cache: Slot<'a, ForceSyncUnpin<RwLock<T>>>,
    ) -> Token<'a> {
        cache.write(ForceSyncUnpin((&mut *f.project_ref().0.get()).0().into()))
    }
}

impl<
        T: Send,
        S: Send + FnMut() -> T,
        M: Send + FnMut(&mut T, T) -> Update,
        SR: SignalRuntimeRef,
    > crate::Source<SR> for RawFold<T, S, M, SR>
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
