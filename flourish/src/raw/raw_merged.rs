use std::{
    borrow::Borrow,
    cell::UnsafeCell,
    mem::{self, size_of},
    pin::Pin,
    sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use pin_project::pin_project;
use pollinate::{
    runtime::{CallbackTableTypes, GlobalSignalRuntime, SignalRuntimeRef, Update},
    slot::{Slot, Token},
    source::{Callbacks, Source},
};

use crate::{traits::Subscribable, utils::conjure_zst};

#[pin_project]
#[must_use = "Signals do nothing unless they are polled or subscribed to."]
pub(crate) struct RawMerged<
    T: Send,
    S: Send + FnMut() -> T,
    M: Send + FnMut(&mut T, T) -> Update,
    SR: SignalRuntimeRef = GlobalSignalRuntime,
>(#[pin] Source<ForceSyncUnpin<UnsafeCell<(S, M)>>, ForceSyncUnpin<RwLock<T>>, SR>);

#[pin_project]
struct ForceSyncUnpin<T: ?Sized>(T);
unsafe impl<T: ?Sized> Sync for ForceSyncUnpin<T> {}

struct RawMergedGuard<'a, T: ?Sized>(RwLockReadGuard<'a, T>);
struct RawMergedGuardExclusive<'a, T: ?Sized>(RwLockWriteGuard<'a, T>);

impl<'a, T: ?Sized> Borrow<T> for RawMergedGuard<'a, T> {
    fn borrow(&self) -> &T {
        self.0.borrow()
    }
}

impl<'a, T: ?Sized> Borrow<T> for RawMergedGuardExclusive<'a, T> {
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
    > Sync for RawMerged<T, S, M, SR>
{
}

/// See [rust-lang#98931](https://github.com/rust-lang/rust/issues/98931).
impl<
        T: Send,
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

    fn get(self: Pin<&Self>) -> T
    where
        T: Sync + Copy,
    {
        if size_of::<T>() == 0 {
            // The read is unobservable, so just skip locking.
            self.touch();
            conjure_zst()
        } else {
            *self.read().borrow()
        }
    }

    fn get_clone(self: Pin<&Self>) -> T
    where
        T: Sync + Clone,
    {
        self.read().borrow().clone()
    }

    pub fn read<'a>(self: Pin<&'a Self>) -> impl 'a + Borrow<T>
    where
        T: Sync,
    {
        RawMergedGuard(self.touch().read().unwrap())
    }

    pub fn read_exclusive<'a>(self: Pin<&'a Self>) -> impl 'a + Borrow<T> {
        RawMergedGuardExclusive(self.touch().write().unwrap())
    }

    fn get_exclusive(self: Pin<&Self>) -> T
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

    fn get_clone_exclusive(self: Pin<&Self>) -> T
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

    pub fn pull<'a>(self: Pin<&'a Self>) -> impl 'a + Borrow<T> {
        unsafe {
            //TODO: SAFETY COMMENT.
            mem::transmute::<RawMergedGuard<T>, RawMergedGuard<T>>(RawMergedGuard(
                self.project_ref()
                    .0
                    .pull_or_init::<E>(|f, cache| Self::init(f, cache))
                    .1
                     .0
                    .read()
                    .unwrap(),
            ))
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
            source: Pin<&Source<ForceSyncUnpin<UnsafeCell<(S, M)>>, ForceSyncUnpin<RwLock<T>>, SR>>,
            eager: Pin<&ForceSyncUnpin<UnsafeCell<(S, M)>>>,
            lazy: Pin<&ForceSyncUnpin<RwLock<T>>>,
            subscribed: <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
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
        T: Send,
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
        Self::Value: Sync,
    {
        Box::new(self.read())
    }

    fn read_exclusive<'a>(self: Pin<&'a Self>) -> Box<dyn 'a + Borrow<Self::Value>> {
        Box::new(self.read_exclusive())
    }

    fn clone_runtime_ref(&self) -> SR
    where
        SR: Sized,
    {
        self.0.clone_runtime_ref()
    }
}

impl<
        T: Send,
        S: Send + FnMut() -> T,
        M: Send + FnMut(&mut T, T) -> Update,
        SR: SignalRuntimeRef,
    > Subscribable<SR> for RawMerged<T, S, M, SR>
{
    fn pull<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Borrow<Self::Value>> {
        Box::new(self.pull())
    }

    fn unsubscribe(self: Pin<&Self>) -> bool {
        self.project_ref().0.unsubscribe()
    }
}
