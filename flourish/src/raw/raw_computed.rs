use std::{
    borrow::Borrow,
    mem::{needs_drop, size_of},
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
pub struct RawComputed<
    T: Send + Clone,
    S: crate::Source<SR, Value = T>,
    SR: SignalRuntimeRef = GlobalSignalRuntime,
>(#[pin] Source<ForceSyncUnpin<S>, ForceSyncUnpin<RwLock<T>>, SR>);

#[pin_project]
struct ForceSyncUnpin<T: ?Sized>(#[pin] T);
unsafe impl<T: ?Sized> Sync for ForceSyncUnpin<T> {}

pub struct RawComputedGuard<'a, T: ?Sized>(RwLockReadGuard<'a, T>);

impl<'a, T: ?Sized> Deref for RawComputedGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.borrow()
    }
}

impl<'a, T: ?Sized> Borrow<T> for RawComputedGuard<'a, T> {
    fn borrow(&self) -> &T {
        self.0.borrow()
    }
}

/// TODO: Safety documentation.
unsafe impl<T: Send + Clone, S: crate::Source<SR, Value = T>, SR: SignalRuntimeRef + Sync> Sync
    for RawComputed<T, S, SR>
{
}

impl<T: Send + Clone, S: crate::Source<SR, Value = T>, SR: SignalRuntimeRef> RawComputed<T, S, SR> {
    pub fn new(source: S) -> Self {
        let runtime = source.clone_runtime_ref();
        Self::with_runtime(source, runtime)
    }

    pub fn with_runtime(source: S, runtime: SR) -> Self
    where
        S: Sized,
        SR: SignalRuntimeRef,
    {
        Self(Source::with_runtime(ForceSyncUnpin(source.into()), runtime))
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

    pub fn read<'a>(self: Pin<&'a Self>) -> RawComputedGuard<'a, T>
    where
        T: Sync,
    {
        let touch = unsafe { Pin::into_inner_unchecked(self.touch()) };
        RawComputedGuard(touch.read().unwrap())
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

    pub(crate) fn touch<'a>(self: Pin<&Self>) -> Pin<&RwLock<T>> {
        unsafe {
            self.project_ref()
                .0
                .project_or_init::<E>(|source, cache| Self::init(source, cache))
                .1
                .project_ref()
                .0
        }
    }

    pub(crate) fn pull(self: Pin<&Self>) -> Pin<&RwLock<T>> {
        unsafe {
            self.project_ref()
                .0
                .pull_or_init::<E>(|source, cache| Self::init(source, cache))
                .1
                .project_ref()
                .0
        }
    }
}

enum E {}
impl<T: Send + Clone, S: crate::Source<SR, Value = T>, SR: SignalRuntimeRef>
    Callbacks<ForceSyncUnpin<S>, ForceSyncUnpin<RwLock<T>>, SR> for E
{
    const UPDATE: Option<
        unsafe fn(eager: Pin<&ForceSyncUnpin<S>>, lazy: Pin<&ForceSyncUnpin<RwLock<T>>>) -> Update,
    > = {
        unsafe fn eval<T: Send + Clone, S: crate::Source<SR, Value = T>, SR: SignalRuntimeRef>(
            source: Pin<&ForceSyncUnpin<S>>,
            cache: Pin<&ForceSyncUnpin<RwLock<T>>>,
        ) -> Update {
            //FIXME: This can be split up to avoid congestion where not necessary.
            let new_value = source.project_ref().0.get_clone_exclusive();
            if needs_drop::<T>() || size_of::<T>() > 0 {
                *cache.project_ref().0.write().unwrap() = new_value;
            } else {
                // The write is unobservable, so just skip locking.
            }
            Update::Propagate
        }
        Some(eval)
    };

    const ON_SUBSCRIBED_CHANGE: Option<
        unsafe fn(
            eager: Pin<&ForceSyncUnpin<S>>,
            lazy: Pin<&ForceSyncUnpin<RwLock<T>>>,
            subscribed: bool,
        ),
    > = None;
}

/// # Safety
///
/// These are the only functions that access `cache`.
/// Externally synchronised through guarantees on [`pollinate::init`].
impl<T: Send + Clone, S: crate::Source<SR, Value = T>, SR: SignalRuntimeRef> RawComputed<T, S, SR> {
    unsafe fn init<'a>(
        source: Pin<&'a ForceSyncUnpin<S>>,
        cache: Slot<'a, ForceSyncUnpin<RwLock<T>>>,
    ) -> Token<'a> {
        cache.write(ForceSyncUnpin(
            //FIXME: This can be split up to avoid congestion where not necessary.
            source.project_ref().0.get_clone_exclusive().into(),
        ))
    }
}

impl<T: Send + Clone, S: crate::Source<SR, Value = T>, SR: SignalRuntimeRef> crate::Source<SR>
    for RawComputed<T, S, SR>
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
