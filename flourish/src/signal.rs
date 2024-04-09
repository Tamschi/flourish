use std::{
    borrow::Borrow,
    cell::UnsafeCell,
    mem::MaybeUninit,
    ops::Deref,
    pin::Pin,
    sync::{Arc, OnceLock, RwLock, RwLockReadGuard},
};

use pin_project::{pin_project, pinned_drop};
use pollinate::{GetPinNonNullExt, SelfHandle};

#[pin_project(PinnedDrop)]
pub struct Signal<F: Send + FnMut() -> T, T: Send + Sync> {
    handle: OnceLock<SelfHandle>,
    #[pin]
    state: UnsafeCell<SignalState<F, T>>,
}

#[pin_project]
struct SignalState<F, T> {
    state: UnsafeCell<F>,
    /// Sadly, this cannot be created poisoned (which would simplify the code).
    cache: MaybeUninit<RwLock<T>>,
}

pub struct SignalGuard<'a, T>(RwLockReadGuard<'a, T>);

impl<'a, T> Deref for SignalGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.borrow()
    }
}

/// TODO: Safety documentation.
unsafe impl<F: Send + FnMut() -> T, T: Send + Sync> Sync for Signal<F, T> {}

impl<F: Send + FnMut() -> T, T: Send + Sync> Signal<F, T> {
    pub fn new(f: F) -> Pin<Arc<Self>> {
        Arc::pin(Self::new_raw(f))
    }

    pub fn new_raw(f: F) -> Self {
        Self {
            handle: OnceLock::new(),
            state: SignalState {
                state: f.into(),
                cache: MaybeUninit::uninit().into(),
            }
            .into(),
        }
    }

    pub fn get(self: Pin<&Self>) -> T
    where
        T: Copy,
    {
        *self.read()
    }

    pub fn get_clone(self: Pin<&Self>) -> T
    where
        T: Clone,
    {
        self.read().clone()
    }

    pub fn read<'a>(self: Pin<&'a Self>) -> SignalGuard<'a, T> {
        pollinate::tag(self.handle.get_or_init(|| unsafe {
            pollinate::init(
                self.project_ref().state.get_pin_non_null(),
                SignalState::init,
                SignalState::eval,
            )
        }));
        SignalGuard(unsafe { (&*self.state.get()).cache.assume_init_ref().read().unwrap() })
    }
}

/// # Safety
///
/// These are the only functions that access `self.state`.
/// Externally synchronised through guarantees on [`pollinate::init`].
impl<F: Send + FnMut() -> T, T: Send + Sync> SignalState<F, T> {
    unsafe extern "C" fn init(self: Pin<&mut Self>) {
        let value = (&mut *self.state.get())();
        self.project().cache.write(RwLock::new(value));
    }

    unsafe extern "C" fn eval(self: Pin<&Self>) {
        *self.cache.assume_init_ref().write().unwrap() = (&mut *self.state.get())();
    }

    unsafe fn drop_cached(&mut self) {
        self.cache.assume_init_drop();
    }
}

#[pinned_drop]
impl<F: Send + FnMut() -> T, T: Send + Sync> PinnedDrop for Signal<F, T> {
    fn drop(mut self: Pin<&mut Self>) {
        if self.handle.take().map(drop).is_some() {
            unsafe {
                // SAFETY: By [`pollinate::init`]'s guarantees,
                // the inner pointer is discarded once `self.handle` is dropped.
                self.state.get_mut().drop_cached()
            }
        }
    }
}
