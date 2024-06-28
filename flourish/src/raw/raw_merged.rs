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
pub(crate) struct RawMerged<
    T: Send + Clone,
    S: Send + FnMut() -> T,
    M: Send + FnMut(&mut T, T) -> Update,
    SR: SignalRuntimeRef = GlobalSignalRuntime,
>(#[pin] Source<ForceSyncUnpin<UnsafeCell<(S, M)>>, ForceSyncUnpin<RwLock<T>>, SR>);

#[pin_project]
struct ForceSyncUnpin<T: ?Sized>(T);
unsafe impl<T: ?Sized> Sync for ForceSyncUnpin<T> {}

pub(crate) struct RawMergedGuard<'a, T: ?Sized>(RwLockReadGuard<'a, T>);

impl<'a, T: ?Sized> Deref for RawMergedGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.borrow()
    }
}

impl<'a, T: ?Sized> Borrow<T> for RawMergedGuard<'a, T> {
    fn borrow(&self) -> &T {
        self.0.borrow()
    }
}

/// TODO: Safety documentation.
unsafe impl<
        T: Send + Clone,
        S: Send + FnMut() -> T,
        M: Send + FnMut(&mut T, T) -> Update,
        SR: SignalRuntimeRef + Sync,
    > Sync for RawMerged<T, S, M, SR>
{
}

/// See [rust-lang#98931](https://github.com/rust-lang/rust/issues/98931).
impl<
        T: Send + Clone,
        S: Send + FnMut() -> T,
        M: Send + FnMut(&mut T, T) -> Update,
        SR: SignalRuntimeRef,
    > RawMerged<T, S, M, SR>
{
    pub fn new(select: S, merge: M, runtime: SR) -> Self {
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

    pub fn read<'a>(self: Pin<&'a Self>) -> RawMergedGuard<'a, T>
    where
        T: Sync,
    {
        RawMergedGuard(self.touch().read().unwrap())
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
                .project_or_init::<E>(|state, cache| Self::init(state, cache))
                .1
                .project_ref()
                .0
        }
    }
}

enum E {}
impl<
        T: Send + Clone,
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
            T: Send + Clone,
            S: Send + FnMut() -> T,
            M: Send + ?Sized + FnMut(&mut T, T) -> Update,
        >(
            state: Pin<&ForceSyncUnpin<UnsafeCell<(S, M)>>>,
            cache: Pin<&ForceSyncUnpin<RwLock<T>>>,
        ) -> Update {
            let (select, merge) = &mut *state.0.get();
            // TODO: Split this up to avoid congestion where possible.
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
        T: Send + Clone,
        S: Send + FnMut() -> T,
        M: Send + FnMut(&mut T, T) -> Update,
        SR: SignalRuntimeRef,
    > RawMerged<T, S, M, SR>
{
    unsafe fn init<'a>(
        state: Pin<&'a ForceSyncUnpin<UnsafeCell<(S, M)>>>,
        cache: Slot<'a, ForceSyncUnpin<RwLock<T>>>,
    ) -> Token<'a> {
        cache.write(ForceSyncUnpin((&mut *state.0.get()).0().into()))
    }
}

impl<
        T: Send + Clone,
        S: Send + FnMut() -> T,
        M: Send + FnMut(&mut T, T) -> Update,
        SR: SignalRuntimeRef,
    > crate::Source<SR> for RawMerged<T, S, M, SR>
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
