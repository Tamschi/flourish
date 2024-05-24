#![warn(clippy::pedantic)]

use std::{
    cell::UnsafeCell, marker::PhantomPinned, mem, num::NonZeroU64, pin::Pin, ptr::NonNull,
    sync::OnceLock,
};

pub mod slot;
use runtime::{GlobalSignalRuntime, SignalRuntimeRef};
use slot::{Slot, Token};

mod deferred_queue;
mod dirty_queue;
pub mod runtime;
mod work_queue;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct SourceId<SR = GlobalSignalRuntime> {
    id: NonZeroU64,
    sr: SR,
}

impl<SR: SignalRuntimeRef> SourceId<SR> {
    fn new() -> Self
    where
        SR: Default,
    {
        Self::with_runtime(SR::default())
    }

    fn with_runtime(sr: SR) -> Self {
        Self {
            id: sr.next_source_id_number(),
            sr,
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct Source<Eager: Sync + ?Sized, Lazy: Sync, SR: SignalRuntimeRef = GlobalSignalRuntime> {
    handle: SourceId<SR>,
    _pinned: PhantomPinned,
    lazy: OnceLock<Lazy>,
    eager: Eager,
}
impl<SR: SignalRuntimeRef + Unpin> Unpin for Source<(), (), SR> {}

impl<Eager: Sync + ?Sized, Lazy: Sync, SR: SignalRuntimeRef> Source<Eager, Lazy, SR> {
    pub fn new(eager: Eager) -> Self
    where
        Eager: Sized,
        SR: Default,
    {
        Self::with_runtime(eager, SR::default())
    }

    pub fn with_runtime(eager: Eager, sr: SR) -> Self
    where
        Eager: Sized,
    {
        Self {
            //TODO: Relax ordering?
            handle: SourceId::with_runtime(sr),
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

impl<Eager: Sync + ?Sized, Lazy: Sync, SR: SignalRuntimeRef> Drop for Source<Eager, Lazy, SR> {
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
