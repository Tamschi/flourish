use core::{
    fmt::{self, Debug, Formatter},
    marker::PhantomPinned,
    pin::Pin,
};
use std::{
    collections::{btree_map::Entry, BTreeMap},
    mem::{self, MaybeUninit},
    sync::{Mutex, OnceLock},
};

use crate::{
    runtime::{CallbackTable, GlobalSignalRuntime, SignalRuntimeRef, Update},
    slot::{Slot, Token},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct SourceId<SR: SignalRuntimeRef = GlobalSignalRuntime> {
    id: SR::Symbol,
    runtime: SR,
}

impl<SR: SignalRuntimeRef> SourceId<SR> {
    fn with_runtime(runtime: SR) -> Self {
        Self {
            id: runtime.next_id(),
            runtime,
        }
    }

    fn mark<T>(&self, f: impl FnOnce() -> T) -> T {
        self.runtime.reentrant_critical(|| {
            self.runtime.touch(self.id);
            f()
        })
    }

    fn update_dependency_set<T>(&self, f: impl FnOnce() -> T) -> T {
        self.runtime.update_dependency_set(self.id, f)
    }

    unsafe fn start<T, D: ?Sized>(
        &self,
        f: impl FnOnce() -> T,
        callback: *const CallbackTable<D>,
        callback_data: *const D,
    ) -> T {
        self.runtime.start(self.id, f, callback, callback_data)
    }

    fn set_subscription(&self, enabled: bool) {
        self.runtime.set_subscription(self.id, enabled);
    }

    fn update_or_enqueue(&self, f: impl 'static + Send + FnOnce()) {
        self.runtime.update_or_enqueue(self.id, f);
    }

    fn propagate(&self) {
        self.runtime.propagate_from(self.id)
    }

    fn refresh(&self) {
        self.runtime.refresh(self.id);
    }

    fn stop(&self) {
        self.runtime.stop(self.id)
    }
}

pub struct Source<Eager: Sync + ?Sized, Lazy: Sync, SR: SignalRuntimeRef = GlobalSignalRuntime> {
    handle: SourceId<SR>,
    _pinned: PhantomPinned,
    lazy: OnceLock<Lazy>,
    eager: Eager,
}

unsafe impl<Eager: Sync + ?Sized, Lazy: Sync, SR: SignalRuntimeRef> Sync
    for Source<Eager, Lazy, SR>
{
    // Access to `eval` is synchronised through `lazy`.
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

    pub fn with_runtime(eager: Eager, runtime: SR) -> Self
    where
        Eager: Sized,
    {
        Self {
            handle: SourceId::with_runtime(runtime),
            _pinned: PhantomPinned,
            lazy: OnceLock::new(),
            eager: eager.into(),
        }
    }

    pub fn eager_mut(&mut self) -> &mut Eager {
        &mut self.eager
    }

    /// # Safety
    ///
    /// `init` is called exactly once with `receiver` before this function returns for the first time for this instance.
    ///
    /// After `init` returns, `E::eval` may be called any number of times with the state initialised by `init`, but at most once at a time.
    ///
    /// [`Source`]'s [`Drop`] implementation first prevents further `eval` calls and waits for running ones to finish (not necessarily in this order), then drops the `T` in place.
    pub unsafe fn project_or_init<'a, C: Callbacks<Eager, Lazy, SR>>(
        self: Pin<&Self>,
        init: impl for<'b> FnOnce(Pin<&'b Eager>, Slot<'b, Lazy>) -> Token<'b>,
    ) -> (Pin<&Eager>, Pin<&Lazy>) {
        let eager = Pin::new_unchecked(&self.eager);
        let lazy = self.handle.mark(|| {
            self.lazy.get_or_init(|| {
                let mut lazy = MaybeUninit::uninit();
                let init = || drop(init(eager, Slot::new(&mut lazy)));
                let callback_table = match CALLBACK_TABLES.lock().expect("unreachable").entry(
                    CallbackTable {
                        update: C::UPDATE.is_some().then_some(update::<Eager, Lazy, SR, C>),
                        on_subscribed_change: C::ON_SUBSCRIBED_CHANGE
                            .is_some()
                            .then_some(on_subscribed_change::<Eager, Lazy, SR, C>),
                    }
                    .into_erased(),
                ) {
                    Entry::Vacant(v) => {
                        let table = v.key().clone();
                        &**v.insert(Box::pin(table)) as *const _
                    }
                    Entry::Occupied(o) => &**o.get() as *const _,
                };
                self.handle.start(
                    init,
                    callback_table,
                    (Pin::into_inner_unchecked(self) as *const Self).cast(),
                );

                static CALLBACK_TABLES: Mutex<
                    BTreeMap<CallbackTable<()>, Pin<Box<CallbackTable<()>>>>,
                > = Mutex::new(BTreeMap::new());

                unsafe extern "C" fn update<
                    Eager: Sync + ?Sized,
                    Lazy: Sync,
                    SR: SignalRuntimeRef,
                    C: Callbacks<Eager, Lazy, SR>,
                >(
                    this: *const Source<Eager, Lazy, SR>,
                ) -> Update {
                    let this = &*this;
                    C::UPDATE.expect("unreachable")(
                        Pin::new_unchecked(&this.eager),
                        Pin::new_unchecked(this.lazy.get().expect("unreachable")),
                    )
                }

                unsafe extern "C" fn on_subscribed_change<
                    Eager: Sync + ?Sized,
                    Lazy: Sync,
                    SR: SignalRuntimeRef,
                    C: Callbacks<Eager, Lazy, SR>,
                >(
                    this: *const Source<Eager, Lazy, SR>,
                    subscribed: bool,
                ) {
                    let this = &*this;
                    C::ON_SUBSCRIBED_CHANGE.expect("unreachable")(
                        Pin::new_unchecked(&this.eager),
                        Pin::new_unchecked(this.lazy.get().expect("unreachable")),
                        subscribed,
                    )
                }

                unsafe { lazy.assume_init() }
            })
        });
        self.handle.refresh();
        unsafe { mem::transmute((eager, Pin::new_unchecked(lazy))) }
    }

    /// TODO: Naming?
    ///
    /// Acts as [`Self::project_or_init`], but also marks this [`Source`] permanently as subscribed (until dropped).
    ///
    /// # Safety
    ///
    /// This function has the same safety requirements as [`Self::project_or_init`].
    pub unsafe fn pull_or_init<E: Callbacks<Eager, Lazy, SR>>(
        self: Pin<&Self>,
        init: impl for<'b> FnOnce(Pin<&'b Eager>, Slot<'b, Lazy>) -> Token<'b>,
    ) -> (Pin<&Eager>, Pin<&Lazy>) {
        let projected = self.project_or_init::<E>(init);
        self.handle.set_subscription(true);
        projected
    }

    //TODO: Can the lifetime requirement be reduced here?
    //      In theory, the closure only needs to live longer than `Self`, but I'm unsure if that's expressible.
    pub fn update<F: 'static + Send + FnOnce(Pin<&Eager>, Pin<&OnceLock<Lazy>>)>(
        //TODO: Optional `Lazy`!
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

    pub fn update_dependency_set<T, F: FnOnce(Pin<&Eager>, Pin<&Lazy>) -> T>(
        self: Pin<&Self>,
        f: F,
    ) -> T {
        self.handle.update_dependency_set(move || unsafe {
            f(
                Pin::new_unchecked(&self.eager),
                Pin::new_unchecked(match self.lazy.get() {
                    Some(lazy) => lazy,
                    None => panic!("`Source::track` may only be used after initialisation."),
                }),
            )
        })
    }

    pub fn clone_runtime_ref(&self) -> SR
    where
        SR: Sized,
    {
        self.handle.runtime.clone()
    }
}

impl<Eager: Sync + ?Sized, Lazy: Sync, SR: SignalRuntimeRef> Drop for Source<Eager, Lazy, SR> {
    fn drop(&mut self) {
        if self.lazy.get().is_some() {
            self.handle.stop()
        }
    }
}

pub trait Callbacks<Eager: ?Sized, Lazy, SR: SignalRuntimeRef> {
    /// # Safety
    ///
    /// Only called once at a time for each initialised [`Source`].
    ///
    /// **Note:** At least with the default runtime, the stale flag *always* propagates while this is [`None`] or there are no active subscribers.
    const UPDATE: Option<unsafe fn(eager: Pin<&Eager>, lazy: Pin<&Lazy>) -> Update>;

    /// # Safety
    ///
    /// Only called once at a time for each initialised [`Source`], and not concurrently with [`Self::UPDATE`].
    const ON_SUBSCRIBED_CHANGE: Option<
        unsafe fn(eager: Pin<&Eager>, lazy: Pin<&Lazy>, subscribed: bool),
    >;
}

pub enum NoCallbacks {}
impl<Eager: ?Sized, Lazy, SR: SignalRuntimeRef> Callbacks<Eager, Lazy, SR> for NoCallbacks {
    const UPDATE: Option<unsafe fn(eager: Pin<&Eager>, lazy: Pin<&Lazy>) -> Update> = None;
    const ON_SUBSCRIBED_CHANGE: Option<
        unsafe fn(eager: Pin<&Eager>, lazy: Pin<&Lazy>, subscribed: bool),
    > = None;
}
