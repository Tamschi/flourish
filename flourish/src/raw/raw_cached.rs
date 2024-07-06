use std::{
    borrow::Borrow,
    mem::{self, needs_drop, size_of},
    ops::Deref,
    pin::Pin,
    sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use pin_project::pin_project;
use pollinate::{
    raw::{Callbacks, RawSignal},
    runtime::{CallbackTableTypes, SignalRuntimeRef, Update},
    slot::{Slot, Token},
};

use crate::{traits::Subscribable, utils::conjure_zst, Source};

#[pin_project]
#[must_use = "Signals do nothing unless they are polled or subscribed to."]
pub(crate) struct RawCached<T: Send + Clone, S: Subscribable<SR, Value = T>, SR: SignalRuntimeRef>(
    #[pin] RawSignal<ForceSyncUnpin<S>, ForceSyncUnpin<RwLock<T>>, SR>,
);

#[pin_project]
struct ForceSyncUnpin<T: ?Sized>(#[pin] T);
unsafe impl<T: ?Sized> Sync for ForceSyncUnpin<T> {}

pub(crate) struct RawCachedGuard<'a, T: ?Sized>(RwLockReadGuard<'a, T>);
struct RawCachedGuardExclusive<'a, T: ?Sized>(RwLockWriteGuard<'a, T>);

impl<'a, T: ?Sized> Deref for RawCachedGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.borrow()
    }
}

impl<'a, T: ?Sized> Borrow<T> for RawCachedGuard<'a, T> {
    fn borrow(&self) -> &T {
        self.0.borrow()
    }
}

impl<'a, T: ?Sized> Deref for RawCachedGuardExclusive<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.borrow()
    }
}

impl<'a, T: ?Sized> Borrow<T> for RawCachedGuardExclusive<'a, T> {
    fn borrow(&self) -> &T {
        self.0.borrow()
    }
}

// TODO: Safety documentation.
unsafe impl<T: Send + Clone, S: Subscribable<SR, Value = T>, SR: SignalRuntimeRef + Sync> Sync
    for RawCached<T, S, SR>
{
}

impl<T: Send + Clone, S: Subscribable<SR, Value = T>, SR: SignalRuntimeRef> RawCached<T, S, SR> {
    pub fn new(source: S) -> Self {
        let runtime = source.clone_runtime_ref();
        Self(RawSignal::with_runtime(
            ForceSyncUnpin(source.into()),
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
            *self.read().borrow()
        }
    }

    pub fn get_clone(self: Pin<&Self>) -> T
    where
        T: Sync + Clone,
    {
        self.read().borrow().clone()
    }

    pub fn read<'a>(self: Pin<&'a Self>) -> impl 'a + Borrow<T>
    where
        T: Sync,
    {
        let touch = unsafe { Pin::into_inner_unchecked(self.touch()) };
        RawCachedGuard(touch.read().unwrap())
    }

    pub fn read_exclusive<'a>(self: Pin<&'a Self>) -> impl 'a + Borrow<T> {
        let touch = unsafe { Pin::into_inner_unchecked(self.touch()) };
        RawCachedGuardExclusive(touch.write().unwrap())
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

    pub(crate) fn touch(self: Pin<&Self>) -> Pin<&RwLock<T>> {
        unsafe {
            self.project_ref()
                .0
                .project_or_init::<E>(|source, cache| Self::init(source, cache))
                .1
                .project_ref()
                .0
        }
    }

    pub(crate) fn pull(self: Pin<&Self>) -> RawCachedGuard<T> {
        unsafe {
            //TODO: SAFETY COMMENT.
            mem::transmute::<RawCachedGuard<T>, RawCachedGuard<T>>(RawCachedGuard(
                self.project_ref()
                    .0
                    .pull_or_init::<E>(|source, cache| Self::init(source, cache))
                    .1
                    .project_ref()
                    .0
                    .read()
                    .unwrap(),
            ))
        }
    }
}

enum E {}
impl<T: Send + Clone, S: Subscribable<SR, Value = T>, SR: SignalRuntimeRef>
    Callbacks<ForceSyncUnpin<S>, ForceSyncUnpin<RwLock<T>>, SR> for E
{
    const UPDATE: Option<
        fn(eager: Pin<&ForceSyncUnpin<S>>, lazy: Pin<&ForceSyncUnpin<RwLock<T>>>) -> Update,
    > = {
        fn eval<T: Send + Clone, S: Subscribable<SR, Value = T>, SR: SignalRuntimeRef>(
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
        fn(
            source: Pin<&RawSignal<ForceSyncUnpin<S>, ForceSyncUnpin<RwLock<T>>, SR>>,
            eager: Pin<&ForceSyncUnpin<S>>,
            lazy: Pin<&ForceSyncUnpin<RwLock<T>>>,
            subscribed: <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
        ),
    > = None;
}

/// # Safety
///
/// These are the only functions that access `cache`.
/// Externally synchronised through guarantees on [`pollinate::init`].
impl<T: Send + Clone, S: Subscribable<SR, Value = T>, SR: SignalRuntimeRef> RawCached<T, S, SR> {
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

impl<T: Send + Clone, S: Subscribable<SR, Value = T>, SR: SignalRuntimeRef> Source<SR>
    for RawCached<T, S, SR>
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

impl<T: Send + Clone, S: Subscribable<SR, Value = T>, SR: SignalRuntimeRef> Subscribable<SR>
    for RawCached<T, S, SR>
{
    fn pull<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Borrow<Self::Value>> {
        Box::new(self.pull())
    }

    fn unsubscribe(self: Pin<&Self>) -> bool {
        self.project_ref().0.unsubscribe()
    }
}
