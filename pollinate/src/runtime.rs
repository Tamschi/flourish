use core::{
    fmt::Debug,
    num::NonZeroU64,
    sync::atomic::{AtomicU64, Ordering},
};
use std::{
    borrow::BorrowMut,
    cell::{RefCell, RefMut},
    collections::{btree_map::Entry, BTreeMap, BTreeSet, VecDeque},
    convert::identity,
    future::Future,
    mem,
    panic::{catch_unwind, resume_unwind, AssertUnwindSafe},
    sync::{Arc, Mutex, Weak},
};

use async_lock::OnceCell;
use parking_lot::{Once, ReentrantMutex, ReentrantMutexGuard};
use scopeguard::ScopeGuard;
use stale_queue::{SensorNotification, StaleQueue};

mod deferred_queue;
mod stale_queue;

pub trait SignalRuntimeRef: Send + Sync + Clone {
    type Symbol: Clone + Copy + Send;
    type CallbackTableTypes: ?Sized + CallbackTableTypes;

    fn next_id(&self) -> Self::Symbol;
    fn reentrant_critical<T>(&self, f: impl FnOnce() -> T) -> T;
    fn touch(&self, id: Self::Symbol);
    unsafe fn start<T, D: ?Sized>(
        &self,
        id: Self::Symbol,
        f: impl FnOnce() -> T,
        callback_table: *const CallbackTable<D, Self::CallbackTableTypes>,
        callback_data: *const D,
    ) -> T;
    fn update_dependency_set<T>(&self, id: Self::Symbol, f: impl FnOnce() -> T) -> T;
    /// # Returns
    ///
    /// Whether there was a change.
    fn set_subscription(&self, id: Self::Symbol, enabled: bool) -> bool;
    fn update_or_enqueue(&self, id: Self::Symbol, f: impl 'static + Send + FnOnce());
    fn update_async<T: Send, F: Send + FnOnce() -> T>(
        &self,
        id: Self::Symbol,
        f: F,
    ) -> impl Send + Future<Output = Result<T, F>>;
    fn update_blocking(&self, id: Self::Symbol, f: impl FnOnce());
    fn propagate_from(&self, id: Self::Symbol);
    fn run_detached<T>(&self, f: impl FnOnce() -> T) -> T;
    fn refresh(&self, id: Self::Symbol);
    fn stop(&self, id: Self::Symbol);
}

#[derive(Default)]
struct ASignalRuntime {
    source_counter: AtomicU64,
    critical_mutex: ReentrantMutex<RefCell<ASignalRuntime_>>,
}

unsafe impl Sync for ASignalRuntime {}

#[derive(Default)]
struct ASignalRuntime_ {
    stale_queue: StaleQueue<ASymbol>,
    context_stack: Vec<Option<(ASymbol, BTreeSet<ASymbol>)>>,
    callbacks: BTreeMap<ASymbol, (*const CallbackTable<(), ACallbackTableTypes>, *const ())>,
    ///FIXME: This is not-at-all a fair queue.
    update_queue: RefCell<VecDeque<(ASymbol, Box<dyn 'static + Send + FnOnce()>)>>,
}

impl ASignalRuntime_ {
    const fn new() -> Self {
        Self {
            stale_queue: StaleQueue::new(),
            context_stack: Vec::new(),
            callbacks: BTreeMap::new(),
            update_queue: RefCell::new(VecDeque::new()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct ASymbol(NonZeroU64);

impl ASignalRuntime {
    const fn new() -> Self {
        Self {
            source_counter: AtomicU64::new(0),
            critical_mutex: ReentrantMutex::new(RefCell::new(ASignalRuntime_::new())),
        }
    }

    #[must_use]
    fn notify_all<'a: 'b, 'b>(
        lock: &'a ReentrantMutexGuard<RefCell<ASignalRuntime_>>,
        notifications: impl IntoIterator<Item = SensorNotification<ASymbol>>,
        mut borrow: RefMut<'b, ASignalRuntime_>,
    ) -> RefMut<'b, ASignalRuntime_> {
        fn notify<'a: 'b, 'b>(
            lock: &'a ReentrantMutexGuard<RefCell<ASignalRuntime_>>,
            SensorNotification { symbol, value }: SensorNotification<ASymbol>,
            mut borrow: RefMut<'b, ASignalRuntime_>,
        ) -> RefMut<'b, ASignalRuntime_> {
            let &(callback_table, data) = borrow.callbacks.get(&symbol).expect("unreachable");
            if let &CallbackTable {
                on_subscribed_change: Some(on_subscribed_change),
                ..
            } = unsafe { &*callback_table }
            {
                //TODO: Dirty queue isolation!
                borrow.context_stack.push(None); // Important! Dependency isolation.
                drop(borrow);
                let r = catch_unwind(|| unsafe { on_subscribed_change(data, value) });
                let mut borrow = (*lock).borrow_mut();
                assert_eq!(borrow.context_stack.pop(), Some(None));
                if let Err(payload) = r {
                    resume_unwind(payload)
                }
                borrow
            } else {
                borrow
            }
        }

        for notification in notifications {
            borrow = notify(&lock, notification, borrow)
        }
        borrow
    }

    fn process_updates_if_ready<'a>(&'a self) {
        let lock = self.critical_mutex.lock();
        let mut borrow = lock.borrow();
        if !borrow.context_stack.is_empty() || borrow.stale_queue.peek().is_some() {
            // Still processing something else (which will) process updates afterwards.
            return;
        }

        while let Some((id, next)) = (|| borrow.update_queue.borrow_mut().pop_front())() {
            debug_assert!(borrow.callbacks.contains_key(&id));
            drop(borrow);
            next();
            self.propagate_from(id);
            borrow = lock.borrow();
        }
    }
}

enum ACallbackTableTypes {}
impl CallbackTableTypes for ACallbackTableTypes {
    type SubscribedStatus = bool;
}

impl SignalRuntimeRef for &ASignalRuntime {
    type Symbol = ASymbol;
    type CallbackTableTypes = ACallbackTableTypes;

    fn next_id(&self) -> Self::Symbol {
        ASymbol(
            //TODO: Relax ordering?
            (self.source_counter.fetch_add(1, Ordering::SeqCst) + 1)
                .try_into()
                .expect("infallible within reasonable time"),
        )
    }

    fn reentrant_critical<T>(&self, f: impl FnOnce() -> T) -> T {
        let _guard = self.critical_mutex.lock();
        f()
    }

    fn touch(&self, id: Self::Symbol) {
        let lock = self.critical_mutex.lock();
        let mut borrow = (*lock).borrow_mut();
        if let Some(Some((context_id, touched))) = &mut borrow.context_stack.last_mut() {
            if id >= *context_id {
                panic!("Tried to depend on later-created signal. To prevent loops, this isn't possible for now.");
            }
            touched.insert(id);
        }
    }

    unsafe fn start<T, D: ?Sized>(
        &self,
        id: Self::Symbol,
        f: impl FnOnce() -> T,
        callback_table: *const CallbackTable<D, Self::CallbackTableTypes>,
        callback_data: *const D,
    ) -> T {
        let lock = self.critical_mutex.lock();
        {
            let mut borrow = (*lock).borrow_mut();
            borrow.stale_queue.register_id(id);
            borrow.context_stack.push(Some((id, BTreeSet::new())));
        }
        let r = catch_unwind(AssertUnwindSafe(f));
        {
            let mut borrow = (*lock).borrow_mut();
            let (popped_id, touched_dependencies) =
                borrow.context_stack.pop().flatten().expect("unreachable");
            assert_eq!(popped_id, id);
            let notifications = borrow
                .stale_queue
                .update_dependency_set(id, touched_dependencies);
            match borrow.callbacks.entry(id) {
                Entry::Vacant(v) => v.insert((
                    CallbackTable::into_erased_ptr(callback_table),
                    callback_data.cast(),
                )),
                Entry::Occupied(_) => panic!("Can't call `start` again before calling `stop`."),
            };
            let _ = ASignalRuntime::notify_all(&lock, notifications, borrow);
        }
        self.process_updates_if_ready();
        r.unwrap_or_else(|p| resume_unwind(p))
    }

    fn update_dependency_set<T>(&self, id: Self::Symbol, f: impl FnOnce() -> T) -> T {
        let lock = self.critical_mutex.lock();
        {
            let mut borrow = (*lock).borrow_mut();
            borrow.context_stack.push(Some((id, BTreeSet::new())));
        }
        let r = catch_unwind(AssertUnwindSafe(f));
        {
            let mut borrow = (*lock).borrow_mut();
            let (popped_id, touched_dependencies) =
                borrow.context_stack.pop().flatten().expect("unreachable");
            assert_eq!(popped_id, id);
            let notifications = borrow
                .stale_queue
                .update_dependency_set(id, touched_dependencies);
            let _ = ASignalRuntime::notify_all(&lock, notifications, borrow);
        }
        self.process_updates_if_ready();
        r.unwrap_or_else(|p| resume_unwind(p))
    }

    fn set_subscription(&self, id: Self::Symbol, enabled: bool) -> bool {
        let lock = self.critical_mutex.lock();
        let mut borrow = (*lock).borrow_mut();
        let (result, notifications) = borrow.stale_queue.set_subscription(id, enabled);
        let _ = ASignalRuntime::notify_all(&lock, notifications, borrow);
        result
    }

    fn update_or_enqueue(&self, id: Self::Symbol, f: impl 'static + Send + FnOnce()) {
        let lock = self.critical_mutex.lock();
        let borrow = (*lock).borrow();

        if !borrow.callbacks.contains_key(&id) {
            panic!("Tried to update without starting the `pollinate::source::Source` first! (This panic may be sporadic when threading.)")
        }

        borrow
            .update_queue
            .borrow_mut()
            .push_back((id, Box::new(f)));
        drop(borrow);
        self.process_updates_if_ready();
    }

    /// Iff polled, schedules `f` to be run as update to the signal with `id`.
    async fn update_async<T: Send, F: Send + FnOnce() -> T>(
        &self,
        id: Self::Symbol,
        f: F,
    ) -> Result<T, F> {
        //TODO: This needs critical section to safely extend the lifetime of `f`.

        let once = Arc::new(OnceCell::<Mutex<Option<Result<T, F>>>>::new());
        self.update_or_enqueue(id, {
            let weak: Weak<_> = Arc::downgrade(&once);
            let guard = {
                let weak = weak.clone();
                scopeguard::guard(f, |f| {
                    if let Some(once) = weak.upgrade() {
                        once.set_blocking(Some(Err(f)).into())
                            .map_err(|_| ())
                            .expect("unreachable");
                    }
                })
            };
            move || {
                // Allow (rough) cancellation.
                if let Some(once) = weak.upgrade() {
                    once.set_blocking(Some(Ok(ScopeGuard::into_inner(guard)())).into())
                        .map_err(|_| ())
                        .expect("unreachable");
                }
            }
        });

        let t = match identity(once)
            .wait()
            .await
            .lock()
            .expect("unreachable")
            .borrow_mut()
            .take()
        {
            Some(Ok(t)) => t,
            Some(Err(f)) => return Err(f),
            None => unreachable!(),
        };

        // Wait again so that propagation also completes first.
        let once = Arc::new(OnceCell::<()>::new());
        self.update_or_enqueue(id, {
            let guard = scopeguard::guard(Arc::downgrade(&once), |c| {
                if let Some(c) = c.upgrade() {
                    c.set_blocking(()).expect("unreachable");
                }
            });
            move || {
                drop(guard);
            }
        });

        once.wait().await;

        Ok(t)
    }

    fn update_blocking(&self, id: Self::Symbol, f: impl FnOnce()) {
        // This is indirected because the nested function's text size may be relatively large.
        //BLOCKED: Avoid the heap allocation once the `Allocator` API is stabilised.

        fn update_blocking(this: &ASignalRuntime, id: ASymbol, f: Box<dyn '_ + FnOnce()>) {
            let lock = this.critical_mutex.lock();
            let borrow = (*lock).borrow();

            if !borrow.callbacks.contains_key(&id) {
                panic!("Tried to update without starting the `pollinate::source::Source` first! (This panic may be sporadic when threading.)")
            }

            if !(borrow.context_stack.is_empty() && borrow.stale_queue.peek().is_none()) {
                panic!("Called `update_blocking` (via `set_blocking`?) while propagating another update. This would deadlock with a better queue.");
            }

            f();
            drop(borrow);
            this.propagate_from(id);
        }
        update_blocking(self, id, Box::new(f))
    }

    fn propagate_from(&self, id: Self::Symbol) {
        let lock = self.critical_mutex.lock();
        if (*lock)
            .borrow_mut()
            .stale_queue
            .mark_dependents_as_stale(id)
        {
            while let Some(current) = (|| (*lock).borrow_mut().stale_queue.next())() {
                let mut borrow = (*lock).borrow_mut();
                let &(callback_table, data) = borrow.callbacks.get(&current).expect("unreachable");
                if let &CallbackTable {
                    update: Some(update),
                    ..
                } = unsafe { &*callback_table }
                {
                    borrow.context_stack.push(Some((current, BTreeSet::new())));
                    drop(borrow);
                    let update = catch_unwind(|| unsafe { update(data) });
                    let mut borrow = (*lock).borrow_mut();
                    let (popped_id, touched_dependencies) =
                        borrow.context_stack.pop().flatten().expect("unreachable");
                    assert_eq!(popped_id, current);
                    let notifications = borrow
                        .stale_queue
                        .update_dependency_set(current, touched_dependencies);
                    borrow = ASignalRuntime::notify_all(&lock, notifications, borrow);
                    match update {
                        Ok(Update::Propagate) => {
                            let _ = borrow.stale_queue.mark_dependents_as_stale(current);
                        }
                        Ok(Update::Halt) => (),
                        Err(payload) => resume_unwind(payload),
                    }
                } else {
                    // As documented on `Callbacks`.
                    let _ = borrow.stale_queue.mark_dependents_as_stale(current);
                }
            }
        }
        self.process_updates_if_ready();
    }

    fn run_detached<T>(&self, f: impl FnOnce() -> T) -> T {
        let lock = self.critical_mutex.lock();
        let mut borrow = (*lock).borrow_mut();
        //TODO: Dirty queue isolation!
        borrow.context_stack.push(None);
        drop(borrow);
        let r = catch_unwind(AssertUnwindSafe(f));
        let mut borrow = (*lock).borrow_mut();
        assert_eq!(borrow.context_stack.pop(), Some(None));
        drop(borrow);
        self.process_updates_if_ready();
        r.unwrap_or_else(|payload| resume_unwind(payload))
    }

    fn refresh(&self, id: Self::Symbol) {
        let lock = self.critical_mutex.lock();
        {
            let mut borrow = (*lock).borrow_mut();
            let is_stale = borrow.stale_queue.remove_stale(id);
            if is_stale {
                let &(callback_table, data) = borrow.callbacks.get(&id).expect("unreachable");
                if let &CallbackTable {
                    update: Some(update),
                    ..
                } = unsafe { &*callback_table }
                {
                    borrow.context_stack.push(Some((id, BTreeSet::new())));
                    drop(borrow);
                    let r = catch_unwind(|| unsafe { update(data) });
                    let mut borrow = (*lock).borrow_mut();
                    let (popped_id, touched_dependencies) =
                        borrow.context_stack.pop().flatten().expect("unreachable");
                    assert_eq!(popped_id, id);
                    if let Err(payload) = r {
                        resume_unwind(payload)
                    }

                    let notifications = borrow
                        .stale_queue
                        .update_dependency_set(id, touched_dependencies);
                    let _ = ASignalRuntime::notify_all(&lock, notifications, borrow);
                }
            }
        }
        self.process_updates_if_ready();
    }

    fn stop(&self, id: Self::Symbol) {
        let lock = self.critical_mutex.lock();
        let mut borrow = (*lock).borrow_mut();
        if borrow
            .context_stack
            .iter()
            .filter_map(|s| s.as_ref())
            .any(|(symbol, _)| *symbol == id)
        {
            //TODO: Does this need to abort the process?
            panic!("Can't stop symbol while it is executing on the same thread.");
        }
        if borrow.stale_queue.is_subscribed(id) {
            let &(callback_table, data) = borrow
                .callbacks
                .get(&id)
                .expect("Tried to stop callbacks for a symbol that wasn't started.");
            {
                if let &CallbackTable {
                    on_subscribed_change: Some(on_subscribed_change),
                    ..
                } = unsafe { &*callback_table }
                {
                    unsafe { on_subscribed_change(data, false) }
                }
            }
        }
        let notifications = borrow.stale_queue.purge_id(id);
        let mut borrow = ASignalRuntime::notify_all(&lock, notifications, borrow);
        borrow.callbacks.remove(&id);

        borrow
            .update_queue
            .borrow_mut()
            .retain(|(item_id, _)| *item_id != id);
    }
}

static GLOBAL_SIGNAL_RUNTIME: ASignalRuntime = ASignalRuntime::new();

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlobalSignalRuntime;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GSRSymbol(ASymbol);

#[repr(transparent)]
pub struct GlobalCallbackTableTypes(ACallbackTableTypes);
impl CallbackTableTypes for GlobalCallbackTableTypes {
    //SAFETY: Everything here must be the same as for `ACallbackTableTypes`!
    type SubscribedStatus = bool;
}

impl SignalRuntimeRef for GlobalSignalRuntime {
    type Symbol = GSRSymbol;
    type CallbackTableTypes = GlobalCallbackTableTypes;

    fn next_id(&self) -> GSRSymbol {
        GSRSymbol((&GLOBAL_SIGNAL_RUNTIME).next_id())
    }

    fn reentrant_critical<T>(&self, f: impl FnOnce() -> T) -> T {
        (&GLOBAL_SIGNAL_RUNTIME).reentrant_critical(f)
    }

    fn touch(&self, id: Self::Symbol) {
        (&GLOBAL_SIGNAL_RUNTIME).touch(id.0)
    }

    unsafe fn start<T, D: ?Sized>(
        &self,
        id: Self::Symbol,
        f: impl FnOnce() -> T,
        callback_table: *const CallbackTable<D, Self::CallbackTableTypes>,
        callback_data: *const D,
    ) -> T {
        (&GLOBAL_SIGNAL_RUNTIME).start(
            id.0,
            f,
            //SAFETY: `GlobalCallbackTableTypes` is deeply transmute-compatible and ABI-compatible to `ACallbackTableTypes`.
            mem::transmute::<
                *const CallbackTable<D, GlobalCallbackTableTypes>,
                *const CallbackTable<D, ACallbackTableTypes>,
            >(callback_table),
            callback_data,
        )
    }

    fn update_dependency_set<T>(&self, id: Self::Symbol, f: impl FnOnce() -> T) -> T {
        (&GLOBAL_SIGNAL_RUNTIME).update_dependency_set(id.0, f)
    }

    fn set_subscription(&self, id: Self::Symbol, enabled: bool) -> bool {
        (&GLOBAL_SIGNAL_RUNTIME).set_subscription(id.0, enabled)
    }

    fn update_or_enqueue(&self, id: Self::Symbol, f: impl 'static + Send + FnOnce()) {
        (&GLOBAL_SIGNAL_RUNTIME).update_or_enqueue(id.0, f)
    }

    async fn update_async<T: Send, F: Send + FnOnce() -> T>(
        &self,
        id: Self::Symbol,
        f: F,
    ) -> Result<T, F> {
        (&GLOBAL_SIGNAL_RUNTIME).update_async(id.0, f).await
    }

    fn update_blocking(&self, id: Self::Symbol, f: impl FnOnce()) {
        (&GLOBAL_SIGNAL_RUNTIME).update_blocking(id.0, f)
    }

    fn propagate_from(&self, id: Self::Symbol) {
        (&GLOBAL_SIGNAL_RUNTIME).propagate_from(id.0)
    }

    fn run_detached<T>(&self, f: impl FnOnce() -> T) -> T {
        (&GLOBAL_SIGNAL_RUNTIME).run_detached(f)
    }

    fn refresh(&self, id: Self::Symbol) {
        (&GLOBAL_SIGNAL_RUNTIME).refresh(id.0)
    }

    fn stop(&self, id: Self::Symbol) {
        (&GLOBAL_SIGNAL_RUNTIME).stop(id.0)
    }
}

#[repr(C)]
#[non_exhaustive]
pub struct CallbackTable<T: ?Sized, CTT: ?Sized + CallbackTableTypes> {
    pub update: Option<unsafe fn(*const T) -> Update>,
    pub on_subscribed_change: Option<unsafe fn(*const T, status: CTT::SubscribedStatus)>,
}

impl<T: ?Sized, CTT: ?Sized + CallbackTableTypes> Debug for CallbackTable<T, CTT> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CallbackTable")
            .field("update", &self.update)
            .field("on_subscribed_change", &self.on_subscribed_change)
            .finish()
    }
}

impl<T: ?Sized, CTT: ?Sized + CallbackTableTypes> Clone for CallbackTable<T, CTT> {
    fn clone(&self) -> Self {
        Self {
            update: self.update.clone(),
            on_subscribed_change: self.on_subscribed_change.clone(),
        }
    }
}

impl<T: ?Sized, CTT: ?Sized + CallbackTableTypes> PartialEq for CallbackTable<T, CTT> {
    fn eq(&self, other: &Self) -> bool {
        self.update == other.update && self.on_subscribed_change == other.on_subscribed_change
    }
}

impl<T: ?Sized, CTT: ?Sized + CallbackTableTypes> Eq for CallbackTable<T, CTT> {}

impl<T: ?Sized, CTT: ?Sized + CallbackTableTypes> PartialOrd for CallbackTable<T, CTT> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.update.partial_cmp(&other.update) {
            Some(core::cmp::Ordering::Equal) => {}
            ord => return ord,
        }
        self.on_subscribed_change
            .partial_cmp(&other.on_subscribed_change)
    }
}

impl<T: ?Sized, CTT: ?Sized + CallbackTableTypes> Ord for CallbackTable<T, CTT> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.update.cmp(&other.update) {
            core::cmp::Ordering::Equal => {}
            ord => return ord,
        }
        self.on_subscribed_change.cmp(&other.on_subscribed_change)
    }
}

pub trait CallbackTableTypes: 'static {
    type SubscribedStatus;
}

impl<T: ?Sized, CTT: ?Sized + CallbackTableTypes> CallbackTable<T, CTT> {
    pub fn into_erased_ptr(this: *const Self) -> *const CallbackTable<(), CTT> {
        unsafe { mem::transmute(this) }
    }

    pub fn into_erased(self) -> CallbackTable<(), CTT> {
        unsafe { mem::transmute(self) }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Update {
    Propagate,
    Halt,
}
