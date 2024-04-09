use std::{
    borrow::Borrow,
    cell::UnsafeCell,
    ops::Deref,
    pin::Pin,
    sync::{Arc, RwLock, RwLockReadGuard},
};

use pin_project::pin_project;
use pollinate::{
    slot::{Slot, Token},
    Source,
};

#[pin_project]
pub struct Signal<F: Send + FnMut() -> T, T: Send>(
    #[pin] Source<ForceSyncUnpin<UnsafeCell<F>>, ForceSyncUnpin<RwLock<T>>>,
);

#[pin_project]
struct ForceSyncUnpin<T>(T);
unsafe impl<T> Sync for ForceSyncUnpin<T> {}

pub struct SignalGuard<'a, T>(RwLockReadGuard<'a, T>);

impl<'a, T> Deref for SignalGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.borrow()
    }
}

impl<'a, T> Borrow<T> for SignalGuard<'a, T> {
    fn borrow(&self) -> &T {
        self.0.borrow()
    }
}

/// TODO: Safety documentation.
unsafe impl<F: Send + FnMut() -> T, T: Send> Sync for Signal<F, T> {}

impl<F: Send + FnMut() -> T, T: Send> Signal<F, T> {
    pub fn new(f: F) -> Pin<Arc<Self>> {
        Arc::pin(Self::new_raw(f))
    }

    pub fn new_raw(f: F) -> Self {
        Self(Source::new(ForceSyncUnpin(f.into())))
    }

    pub fn get(self: Pin<&Self>) -> T
    where
        T: Sync + Copy,
    {
        *self.read()
    }

    pub fn get_clone(self: Pin<&Self>) -> T
    where
        T: Sync + Clone,
    {
        self.read().clone()
    }

    pub fn read<'a>(self: Pin<&'a Self>) -> SignalGuard<'a, T>
    where
        T: Sync,
    {
        SignalGuard(unsafe { self.touch().read().unwrap() })
    }

    pub fn get_exclusive(self: Pin<&Self>) -> T
    where
        T: Copy,
    {
        self.get_clone_exclusive()
    }

    pub fn get_clone_exclusive(self: Pin<&Self>) -> T
    where
        T: Clone,
    {
        unsafe { self.touch().write().unwrap().clone() }
    }

    fn touch(self: Pin<&Self>) -> &RwLock<T> {
        self.0.tag();
        unsafe {
            self.project_ref()
                .0
                .project_or_init(|f, cache| unsafe { Self::init(f, cache) }, Some(Self::eval))
                .1
                .project_ref()
                .0
        }
    }
}

/// # Safety
///
/// These are the only functions that access `cache`.
/// Externally synchronised through guarantees on [`pollinate::init`].
impl<F: Send + FnMut() -> T, T: Send> Signal<F, T> {
    unsafe fn init<'a>(
        f: Pin<&'a ForceSyncUnpin<UnsafeCell<F>>>,
        cache: Slot<'a, ForceSyncUnpin<RwLock<T>>>,
    ) -> Token<'a> {
        cache.write(unsafe { ForceSyncUnpin(f.project_ref().0.get_mut()().into()) })
    }

    unsafe extern "C" fn eval(
        f: Pin<&ForceSyncUnpin<UnsafeCell<F>>>,
        cache: Pin<&ForceSyncUnpin<RwLock<T>>>,
    ) {
        *cache.project_ref().0.write().unwrap() = f.project_ref().0.get_mut()();
    }
}
