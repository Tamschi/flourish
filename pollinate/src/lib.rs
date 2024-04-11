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

pub mod slot;
use slot::{Slot, Token};

mod deferred_queue;
mod dirty_queue;
mod work_queue;

static SOURCE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct SourceId(NonZeroU64);

impl SourceId {
    fn new() -> Self {
        Self(
            (SOURCE_COUNTER.fetch_add(1, Ordering::SeqCst) + 1)
                .try_into()
                .expect("infallible within reasonable time"),
        )
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct Source<Eager: Sync + ?Sized, Lazy: Sync> {
    handle: SourceId,
    _pinned: PhantomPinned,
    lazy: OnceLock<Lazy>,
    eager: Eager,
}
impl Unpin for Source<(), ()> {}

impl<Eager: Sync + ?Sized, Lazy: Sync> Source<Eager, Lazy> {
    pub fn new(eager: Eager) -> Self
    where
        Eager: Sized,
    {
        Self {
            //TODO: Relax ordering?
            handle: SourceId::new(),
            _pinned: PhantomPinned,
            eager: eager.into(),
            lazy: OnceLock::new(),
        }
    }

    pub fn eager_mut(&mut self) -> &mut Eager {
        &mut self.eager
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

    //TODO: Can the lifetime requirement be reduced here?
    //      In theory, the closure only needs to live longer than `Self`, but I'm unsure if that's expressible.
    pub fn update<F: 'static + Send + FnOnce(Pin<&Eager>, Pin<&OnceLock<Lazy>>)>(
        self: Pin<&Self>,
        f: F,
    ) {
        todo!()
    }

    pub fn update_blocking<F: FnOnce(Pin<&Eager>, Pin<&OnceLock<Lazy>>)>(&self, f: F) {
        todo!()
    }
}

impl<Eager: Sync + ?Sized, Lazy: Sync> Drop for Source<Eager, Lazy> {
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
