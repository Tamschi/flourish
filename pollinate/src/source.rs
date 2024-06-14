use core::{
    fmt::{self, Debug, Formatter},
    marker::PhantomPinned,
    pin::Pin,
};
use std::{
    mem::{self, MaybeUninit},
    sync::OnceLock,
};

use crate::{
    runtime::{GlobalSignalRuntime, SignalRuntimeRef},
    slot::{Slot, Token},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct SourceId<SR: SignalRuntimeRef = GlobalSignalRuntime> {
    id: SR::Symbol,
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
            id: sr.next_id(),
            sr,
        }
    }

    fn mark<T>(&self, f: impl FnOnce() -> T) -> T {
        self.sr.reentrant_critical(|| {
            self.sr.touch(self.id);
            f()
        })
    }

    unsafe fn start<T, D: ?Sized>(
        &self,
        f: impl FnOnce() -> T,
        callback: unsafe extern "C" fn(*const D),
        callback_data: *const D,
    ) -> T {
        self.sr.start(self.id, f, callback, callback_data)
    }

    fn set_subscription(&self, enabled: bool) {
        self.sr.set_subscription(self.id, enabled);
    }

    fn update_or_enqueue(&self, f: impl 'static + Send + FnOnce()) {
        self.sr.update_or_enqueue(self.id, f);
    }

    fn propagate(&self) {
        self.sr.propagate_from(self.id)
    }

    fn stop(&self) {
        self.sr.stop(self.id)
    }
}

pub struct Source<Eager: Sync + ?Sized, Lazy: Sync, SR: SignalRuntimeRef = GlobalSignalRuntime> {
    handle: SourceId<SR>,
    _pinned: PhantomPinned,
    lazy: OnceLock<Lazy>,
    eager: Eager,
}

impl<Eager: Sync + ?Sized + Debug, Lazy: Sync + Debug, SR: SignalRuntimeRef + Debug> Debug
    for Source<Eager, Lazy, SR>
where
    SR::Symbol: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Source")
            .field("handle", &self.handle)
            .field("_pinned", &self._pinned)
            .field("lazy", &self.lazy)
            .field("eager", &&self.eager)
            .finish()
    }
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
        let eager = Pin::new_unchecked(&self.eager);
        let lazy = self.handle.mark(|| {
            self.lazy.get_or_init(|| {
                let mut lazy = MaybeUninit::uninit();
                let init = || drop(init(eager, Slot::new(&mut lazy)));
                if let Some(eval) = eval {
                    self.handle
                        .start(init, callback, Pin::into_inner_unchecked(self) as *const _);
                    unsafe extern "C" fn callback<
                        Eager: Sync + ?Sized,
                        Lazy: Sync,
                        SR: SignalRuntimeRef,
                    >(
                        this: *const Source<Eager, Lazy, SR>,
                    ) {
                        todo!()
                    }
                } else {
                    init()
                }
                unsafe { lazy.assume_init() }
            })
        });
        unsafe { mem::transmute((eager, Pin::new_unchecked(lazy))) }
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
        let projected = self.project_or_init(init, eval);
        self.handle.set_subscription(true);
        projected
    }

    //TODO: Can the lifetime requirement be reduced here?
    //      In theory, the closure only needs to live longer than `Self`, but I'm unsure if that's expressible.
    pub fn update<F: 'static + Send + FnOnce(Pin<&Eager>, Pin<&OnceLock<Lazy>>)>(
        self: Pin<&Self>,
        f: F,
    ) where
        SR: 'static + Sync,
        SR::Symbol: Sync,
        Lazy: 'static + Send,
    {
        let this = Pin::clone(&self);
        let update: Box<dyn Send + FnOnce()> = Box::new(move || unsafe {
            f(
                this.map_unchecked(|this| &this.eager),
                this.map_unchecked(|this| &this.lazy),
            )
        });
        let update: Box<dyn Send + FnOnce()> = unsafe { mem::transmute(update) };
        self.handle.update_or_enqueue(update);
    }

    pub fn update_blocking<F: FnOnce(Pin<&Eager>, Pin<&OnceLock<Lazy>>)>(&self, f: F) {
        todo!("This should be in a critical section too.");
        unsafe {
            f(
                Pin::new_unchecked(&self.eager),
                Pin::new_unchecked(&self.lazy),
            );
        }
        self.handle.propagate()
    }
}

impl<Eager: Sync + ?Sized, Lazy: Sync, SR: SignalRuntimeRef> Drop for Source<Eager, Lazy, SR> {
    fn drop(&mut self) {
        self.handle.stop()
    }
}
