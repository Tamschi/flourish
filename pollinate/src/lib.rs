#![warn(clippy::pedantic)]

use std::{
    cell::UnsafeCell,
    marker::PhantomPinned,
    mem,
    num::NonZeroU64,
    pin::Pin,
    ptr::NonNull,
    sync::{
        atomic::{AtomicU64, Ordering},
        OnceLock,
    },
};

use slot::{Slot, Token};

static SOURCE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct Source<Eager: Sync, Lazy: Sync> {
    handle: NonZeroU64,
    _pinned: PhantomPinned,
    eager: Eager,
    lazy: OnceLock<Lazy>,
}
impl Unpin for Source<(), ()> {}

pub mod slot;

impl<Eager: Sync, Lazy: Sync> Source<Eager, Lazy> {
    pub fn new(eager: Eager) -> Self {
        Self {
            //TODO: Relax ordering?
            handle: (SOURCE_COUNTER.fetch_add(1, Ordering::SeqCst) + 1)
                .try_into()
                .expect("infallible within reasonable time"),
            _pinned: PhantomPinned,
            eager: eager.into(),
            lazy: OnceLock::new(),
        }
    }

    /// # Safety
    ///
    /// `init` is called exactly once with `receiver` before this function returns for the first time for this instance.
    ///
    /// After `init` returns, `eval` may be called any number of times with the state initialised by `init`, but at most once at a time.
    ///
    /// [`Source`]'s [`Drop`] implementation first prevents further `eval` calls and waits for running ones to finish (not necessarily in this order), then drops the `T` in place.
    pub unsafe fn project_or_init(
        self: Pin<&Self>,
        init: impl for<'b> FnOnce(Pin<&'b Eager>, Slot<'b, Lazy>) -> Token<'b>,
        eval: Option<unsafe extern "C" fn(eager: Pin<&Eager>, lazy: Pin<&Lazy>)>,
    ) -> (Pin<&Eager>, Pin<&Lazy>) {
        todo!()
    }

    /// TODO: Naming?
    ///
    /// Acts as [`Self::project_or_init`], but also marks this [`Source`] permanently as subscribed (until dropped).
    ///
    /// # Safety
    ///
    /// This function has the same safety requirements as [`Self::project_or_init`].
    pub unsafe fn pull_or_init(
        self: Pin<&Self>,
        init: impl for<'b> FnOnce(Pin<&'b Eager>, Slot<'b, Lazy>) -> Token<'b>,
        eval: Option<unsafe extern "C" fn(eager: Pin<&Eager>, lazy: Pin<&Lazy>)>,
    ) -> (Pin<&Eager>, Pin<&Lazy>) {
        todo!()
    }

    /// # Panics
    ///
    /// - iff recording dependencies for an earlier or the same signal.
    pub fn tag(&self) {
        todo!()
    }

    //TODO: Can the lifetime requirement be reduced here?
    //      In theory, the closure only needs to live longer than `Self`, but I'm unsure if that's expressible.
    pub fn update<F: 'static + Send + FnOnce(Pin<&Eager>, Pin<&Lazy>)>(self: Pin<&Self>, f: F) {
        todo!()
    }

    pub fn update_blocking<F: FnOnce(Pin<&Eager>, Pin<&Lazy>)>(self: Pin<&Self>, f: F) {
        todo!()
    }
}

impl<Eager: Sync, Lazy: Sync> Drop for Source<Eager, Lazy> {
    fn drop(&mut self) {
        todo!()
    }
}

///TODO: Remove?
pub trait GetPinNonNullExt {
    type Target: ?Sized;
    fn get_pin_non_null(self: Pin<&Self>) -> Pin<NonNull<Self::Target>>;
}

impl<T: ?Sized> GetPinNonNullExt for UnsafeCell<T> {
    type Target = T;

    fn get_pin_non_null(self: Pin<&Self>) -> Pin<NonNull<Self::Target>> {
        let pointer = self.get();
        unsafe {
            // SAFETY: Memory layout guaranteed by `#[repr(transparent)]` on `Pin<…>` and `NonNull<…>`.
            mem::transmute::<*mut T, Pin<NonNull<T>>>(pointer)
        }
    }
}

///TODO: Remove?
pub trait IntoPinNonNullExt {
    fn into_pin_non_null(self: Pin<&mut Self>) -> Pin<NonNull<Self>>;
}

impl<T: ?Sized> IntoPinNonNullExt for T {
    fn into_pin_non_null(self: Pin<&mut Self>) -> Pin<NonNull<Self>> {
        unsafe {
            // SAFETY: Memory layout guaranteed by `#[repr(transparent)]` on `Pin<…>` and `NonNull<…>`.
            mem::transmute::<Pin<&mut T>, Pin<NonNull<T>>>(self)
        }
    }
}
