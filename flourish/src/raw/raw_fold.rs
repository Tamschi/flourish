use std::{
    borrow::Borrow,
    cell::UnsafeCell,
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
pub struct RawFold<
    T: Send,
    F: Send + ?Sized + FnMut(&mut T) -> Update,
    SR: SignalRuntimeRef = GlobalSignalRuntime,
>(#[pin] Source<ForceSyncUnpin<UnsafeCell<(RwLock<T>, F)>>, (), SR>);

#[pin_project]
struct ForceSyncUnpin<T: ?Sized>(T);
unsafe impl<T: ?Sized> Sync for ForceSyncUnpin<T> {}

pub struct RawFoldGuard<'a, T: ?Sized>(RwLockReadGuard<'a, T>);

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
unsafe impl<T: Send, F: Send + FnMut(&mut T) -> Update, SR: SignalRuntimeRef + Sync> Sync
    for RawFold<T, F, SR>
{
}

/// See [rust-lang#98931](https://github.com/rust-lang/rust/issues/98931).
impl<T: Send, F: Send + FnMut(&mut T) -> Update> RawFold<T, F> {
    pub fn new(initial: T, f: F) -> Self {
        Self::with_runtime(initial, f, GlobalSignalRuntime)
    }
}

impl<T: Send, F: Send + ?Sized + FnMut(&mut T) -> Update, SR: SignalRuntimeRef> RawFold<T, F, SR> {
    pub fn with_runtime(initial: T, f: F, sr: SR) -> Self
    where
        F: Sized,
        SR: SignalRuntimeRef,
    {
        Self(Source::with_runtime(
            ForceSyncUnpin((initial.into(), f).into()),
            sr,
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
impl<T: Send, F: Send + ?Sized + FnMut(&mut T) -> Update>
    Callbacks<ForceSyncUnpin<UnsafeCell<(RwLock<T>, F)>>, ()> for E
{
    const UPDATE: Option<
        unsafe fn(
            eager: Pin<&ForceSyncUnpin<UnsafeCell<(RwLock<T>, F)>>>,
            lazy: Pin<&()>,
        ) -> Update,
    > = {
        unsafe fn eval<T: Send, F: Send + ?Sized + FnMut(&mut T) -> Update>(
            f: Pin<&ForceSyncUnpin<UnsafeCell<(RwLock<T>, F)>>>,
            cache: Pin<&()>,
        ) -> Update {
            let new_value = (&mut *f.project_ref().0.get())();
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
            eager: Pin<&ForceSyncUnpin<UnsafeCell<(RwLock<T>, F)>>>,
            lazy: Pin<&()>,
            subscribed: bool,
        ),
    > = None;
}

/// # Safety
///
/// These are the only functions that access `cache`.
/// Externally synchronised through guarantees on [`pollinate::init`].
impl<T: Send, F: Send + ?Sized + FnMut(&mut T) -> Update, SR: SignalRuntimeRef> RawFold<T, F, SR> {
    unsafe fn init<'a>(
        f: Pin<&'a ForceSyncUnpin<UnsafeCell<F>>>,
        cache: Slot<'a, ForceSyncUnpin<RwLock<T>>>,
    ) -> Token<'a> {
        cache.write(ForceSyncUnpin((&mut *f.project_ref().0.get())().into()))
    }
}

#[macro_export]
macro_rules! fold {
	{$runtime:expr=> $(let $(mut $(@@ $_mut:ident)?)? $name:ident => $f:expr;)*} => {$(
		let $name = ::std::pin::pin!($crate::raw::RawFold::with_runtime(|| $f, $runtime));
		let $(mut $(@@ $_mut)?)? $name = $name.into_ref();
	)*};
    {$(let $(mut $(@@ $_mut:ident)?)? $name:ident => $f:expr;)*} => {$(
		let $name = ::std::pin::pin!($crate::raw::RawFold::new(|| $f));
		let $(mut $(@@ $_mut)?)? $name = $name.into_ref();
	)*};
}

impl<T: Send, F: Send + ?Sized + FnMut(&mut T) -> Update, SR: SignalRuntimeRef> crate::Source
    for RawFold<T, F, SR>
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
        Self::Value: Copy,
    {
        self.get_clone_exclusive()
    }

    fn read<'a>(self: Pin<&'a Self>) -> Box<dyn 'a + Borrow<Self::Value>>
    where
        Self::Value: 'a + Sync,
    {
        Box::new(self.read())
    }
}
