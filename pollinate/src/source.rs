use core::{
    fmt::{self, Debug, Formatter},
    marker::PhantomPinned,
    pin::Pin,
};
use std::sync::OnceLock;

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

    fn stop(&self) {
        self.sr.stop(self.id)
    }
}

#[repr(C)]
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
        self.handle.stop()
    }
}
