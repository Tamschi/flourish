use std::{
    borrow::Borrow,
    mem::{self, needs_drop, size_of},
    pin::Pin,
    sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use pin_project::pin_project;
use pollinate::{
    runtime::{GlobalSignalRuntime, SignalRuntimeRef, Update},
    slot::{Slot, Token},
    source::{Callbacks, Source},
};

use crate::{traits::Subscribable, utils::conjure_zst};

#[pin_project]
#[must_use = "Signals do nothing unless they are polled or subscribed to."]
pub(crate) struct RawComputed<
    T: Send,
    F: Send + FnMut() -> T,
    SR: SignalRuntimeRef = GlobalSignalRuntime,
>(#[pin] Source<ForceSyncUnpin<Mutex<F>>, ForceSyncUnpin<RwLock<T>>, SR>);

#[pin_project]
struct ForceSyncUnpin<T: ?Sized>(#[pin] T);
unsafe impl<T: ?Sized> Sync for ForceSyncUnpin<T> {}

struct RawComputedGuard<'a, T: ?Sized>(RwLockReadGuard<'a, T>);
struct RawComputedGuardExclusive<'a, T: ?Sized>(RwLockWriteGuard<'a, T>);

impl<'a, T: ?Sized> Borrow<T> for RawComputedGuard<'a, T> {
    fn borrow(&self) -> &T {
        self.0.borrow()
    }
}

impl<'a, T: ?Sized> Borrow<T> for RawComputedGuardExclusive<'a, T> {
    fn borrow(&self) -> &T {
        self.0.borrow()
    }
}

/// TODO: Safety documentation.
unsafe impl<T: Send, F: Send + FnMut() -> T, SR: SignalRuntimeRef + Sync> Sync
    for RawComputed<T, F, SR>
{
}

impl<T: Send, F: Send + FnMut() -> T, SR: SignalRuntimeRef> RawComputed<T, F, SR> {
    pub fn new(f: F, runtime: SR) -> Self {
        Self(Source::with_runtime(ForceSyncUnpin(f.into()), runtime))
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
        RawComputedGuard(touch.read().unwrap())
    }

    pub fn read_exclusive<'a>(self: Pin<&'a Self>) -> impl 'a + Borrow<T> {
        let touch = unsafe { Pin::into_inner_unchecked(self.touch()) };
        RawComputedGuardExclusive(touch.write().unwrap())
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
                .project_or_init::<E>(|f, cache| Self::init(f, cache))
                .1
                .project_ref()
                .0
        }
    }

    pub fn pull<'a>(self: Pin<&'a Self>) -> impl 'a + Borrow<T> {
        unsafe {
            //TODO: SAFETY COMMENT.
            mem::transmute::<RawComputedGuard<T>, RawComputedGuard<T>>(RawComputedGuard(
                self.project_ref()
                    .0
                    .pull_or_init::<E>(|f, cache| Self::init(f, cache))
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
impl<T: Send, F: Send + FnMut() -> T, SR: SignalRuntimeRef>
    Callbacks<ForceSyncUnpin<Mutex<F>>, ForceSyncUnpin<RwLock<T>>, SR> for E
{
    const UPDATE: Option<
        unsafe fn(
            eager: Pin<&ForceSyncUnpin<Mutex<F>>>,
            lazy: Pin<&ForceSyncUnpin<RwLock<T>>>,
        ) -> Update,
    > = {
        unsafe fn eval<T: Send, F: Send + FnMut() -> T>(
            f: Pin<&ForceSyncUnpin<Mutex<F>>>,
            cache: Pin<&ForceSyncUnpin<RwLock<T>>>,
        ) -> Update {
            //FIXME: This is externally synchronised already.
            let new_value = f.project_ref().0.try_lock().expect("unreachable")();
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
            eager: Pin<&ForceSyncUnpin<Mutex<F>>>,
            lazy: Pin<&ForceSyncUnpin<RwLock<T>>>,
            subscribed: bool,
        ),
    > = None;
}

/// # Safety
///
/// These are the only functions that access `cache`.
/// Externally synchronised through guarantees on [`pollinate::init`].
impl<T: Send, F: Send + FnMut() -> T, SR: SignalRuntimeRef> RawComputed<T, F, SR> {
    unsafe fn init<'a>(
        f: Pin<&'a ForceSyncUnpin<Mutex<F>>>,
        cache: Slot<'a, ForceSyncUnpin<RwLock<T>>>,
    ) -> Token<'a> {
        cache.write(ForceSyncUnpin(
            //FIXME: This is technically already externally synchronised.
            f.project_ref().0.try_lock().expect("unreachable")().into(),
        ))
    }
}

impl<T: Send, F: Send + FnMut() -> T, SR: SignalRuntimeRef> crate::Source<SR>
    for RawComputed<T, F, SR>
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

impl<T: Send, F: Send + FnMut() -> T, SR: SignalRuntimeRef> Subscribable<SR>
    for RawComputed<T, F, SR>
{
    fn pull<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Borrow<Self::Value>> {
        Box::new(self.pull())
    }

    fn unsubscribe(self: Pin<&Self>) -> bool {
        self.project_ref().0.unsubscribe()
    }
}
