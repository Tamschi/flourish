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
    runtime::{GlobalSignalRuntime, SignalRuntimeRef},
    slot::{Slot, Token},
    source::{Eval, Source},
};

use crate::utils::conjure_zst;

#[pin_project]
#[must_use = "Signals do nothing unless they are polled or subscribed to."]
pub struct RawSignal<
    T: Send,
    F: Send + ?Sized + FnMut() -> T,
    SR: SignalRuntimeRef = GlobalSignalRuntime,
>(#[pin] Source<ForceSyncUnpin<UnsafeCell<F>>, ForceSyncUnpin<RwLock<T>>, SR>);

#[pin_project]
struct ForceSyncUnpin<T: ?Sized>(T);
unsafe impl<T: ?Sized> Sync for ForceSyncUnpin<T> {}

pub struct RawSignalGuard<'a, T: ?Sized>(RwLockReadGuard<'a, T>);

impl<'a, T: ?Sized> Deref for RawSignalGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.borrow()
    }
}

impl<'a, T: ?Sized> Borrow<T> for RawSignalGuard<'a, T> {
    fn borrow(&self) -> &T {
        self.0.borrow()
    }
}

/// TODO: Safety documentation.
unsafe impl<T: Send, F: Send + FnMut() -> T, SR: SignalRuntimeRef + Sync> Sync
    for RawSignal<T, F, SR>
{
}

/// See [rust-lang#98931](https://github.com/rust-lang/rust/issues/98931).
impl<T: Send, F: Send + FnMut() -> T> RawSignal<T, F> {
    pub fn new(f: F) -> Self {
        Self::with_runtime(f, GlobalSignalRuntime)
    }
}

impl<T: Send, F: Send + ?Sized + FnMut() -> T, SR: SignalRuntimeRef> RawSignal<T, F, SR> {
    pub fn with_runtime(f: F, sr: SR) -> Self
    where
        F: Sized,
        SR: SignalRuntimeRef,
    {
        Self(Source::with_runtime(ForceSyncUnpin(f.into()), sr))
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

    pub fn read<'a>(self: Pin<&'a Self>) -> RawSignalGuard<'a, T>
    where
        T: Sync,
    {
        RawSignalGuard(self.touch().read().unwrap())
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
impl<T: Send, F: Send + ?Sized + FnMut() -> T>
    Eval<ForceSyncUnpin<UnsafeCell<F>>, ForceSyncUnpin<RwLock<T>>> for E
{
    unsafe fn eval(f: Pin<&ForceSyncUnpin<UnsafeCell<F>>>, cache: Pin<&ForceSyncUnpin<RwLock<T>>>) {
        let new_value = (&mut *f.project_ref().0.get())();
        if needs_drop::<T>() || size_of::<T>() > 0 {
            *cache.project_ref().0.write().unwrap() = new_value;
        } else {
            // The write is unobservable, so just skip locking.
        }
    }
}

/// # Safety
///
/// These are the only functions that access `cache`.
/// Externally synchronised through guarantees on [`pollinate::init`].
impl<T: Send, F: Send + ?Sized + FnMut() -> T, SR: SignalRuntimeRef> RawSignal<T, F, SR> {
    unsafe fn init<'a>(
        f: Pin<&'a ForceSyncUnpin<UnsafeCell<F>>>,
        cache: Slot<'a, ForceSyncUnpin<RwLock<T>>>,
    ) -> Token<'a> {
        cache.write(ForceSyncUnpin((&mut *f.project_ref().0.get())().into()))
    }
}

#[macro_export]
macro_rules! signal {
	{$runtime:expr=> $(let $(mut $(@@ $_mut:ident)?)? $name:ident => $f:expr;)*} => {$(
		let $name = ::std::pin::pin!($crate::raw::RawSignal::with_runtime(|| $f, $runtime));
		let $(mut $(@@ $_mut)?)? $name = $name.into_ref();
	)*};
    {$(let $(mut $(@@ $_mut:ident)?)? $name:ident => $f:expr;)*} => {$(
		let $name = ::std::pin::pin!($crate::raw::RawSignal::new(|| $f));
		let $(mut $(@@ $_mut)?)? $name = $name.into_ref();
	)*};
}

impl<T: Send, F: Send + ?Sized + FnMut() -> T, SR: SignalRuntimeRef> crate::Source
    for RawSignal<T, F, SR>
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
