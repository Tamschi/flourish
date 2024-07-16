//! Low-level types for implementing [`SignalRuntimeRef`], as well as a functional [`GlobalSignalRuntime`].

use core::{
	num::NonZeroU64,
	sync::atomic::{AtomicU64, Ordering},
};
use std::{
	borrow::{Borrow, BorrowMut},
	cell::{RefCell, RefMut},
	collections::{btree_map::Entry, BTreeMap, BTreeSet, VecDeque},
	convert::identity,
	fmt::Debug,
	future::Future,
	mem,
	panic::{catch_unwind, resume_unwind, AssertUnwindSafe},
	sync::{Arc, Mutex, Weak},
};

use async_lock::OnceCell;
use parking_lot::{ReentrantMutex, ReentrantMutexGuard};
use scopeguard::{guard, ScopeGuard};

/// Trait for handles that let signals refer to a specific runtime (instance).
///
/// [`GlobalSignalRuntime`] provides a usable default.
///
/// # Logic
///
/// Callbacks associated with the same `id` **must not** run in parallel or nested.  
/// Callback invocations *with the same `id` **must** be totally orderable across all threads.
///
/// # Safety
///
/// Please see the 'Safety' sections on individual associated items.
pub unsafe trait SignalRuntimeRef: Send + Sync + Clone {
	/// The signal instance key used by this [`SignalRuntimeRef`].
	///
	/// Used to manage dependencies and callbacks.
	type Symbol: Clone + Copy + Send;

	/// Types used in callback signatures.
	type CallbackTableTypes: ?Sized + CallbackTableTypes;

	/// Creates a fresh unique [`SignalRuntimeRef::Symbol`] for this instance.
	///
	/// Symbols are usually not interchangeable between different instances of a runtime!  
	/// Runtimes **should** detect and panic on misuse when debug-assertions are enabled.
	///
	/// # Safety
	///
	/// The return value **must** be able to uniquely identify a signal towards this runtime.  
	/// Symbols **may not** be reused even after calls to [`.stop(id)`](`SignalRuntimeRef::stop`).
	fn next_id(&self) -> Self::Symbol;

	/// When run in a context that records dependencies, records `id` as dependency of that context.
	///
	/// # Logic
	///
	/// If a touch causes a subscription change, the runtime **should** call that [`CallbackTable::on_subscribed_change`]
	/// callback before returning from this function. (This helps more easily manage on-demand-only resources.)
	///
	/// This method **must** function even for a unknown `id`.
	fn record_dependency(&self, id: Self::Symbol);

	/// Starts managed callback processing for `id`.
	///
	/// # Logic
	///
	/// Dependencies that are [recorded](`SignalRuntimeRef::record_dependency`) within
	/// `init` and [`CallbackTable::update`] on the same thread **must** be recorded
	/// as and update the dependency set of `id`, respectively.
	///
	/// The [`CallbackTable::on_subscribed_change`] callback **must** run detached from
	/// outer dependency recording.
	///
	/// # Safety
	///
	/// Before this method returns, `f` **must** be called.
	///
	/// Only after `f` completes, the runtime **may** run the functions specified in `callback_table` with
	/// `callback_data`, but only one at a time and only before the next [`.stop(id)`](`SignalRuntimeRef::stop`)
	/// call for the same runtime with an identical `id` completes.
	///
	/// # See also
	///
	/// [`SignalRuntimeRef::stop`], [`SignalRuntimeRef::purge`]
	unsafe fn start<T, D: ?Sized>(
		&self,
		id: Self::Symbol,
		init: impl FnOnce() -> T,
		callback_table: *const CallbackTable<D, Self::CallbackTableTypes>,
		callback_data: *const D,
	) -> T;

	/// Removes callbacks associated with `id`.
	///
	/// # Logic
	///
	/// This method **should not** remove interdependencies, just clear the callback information.
	///
	/// # Safety
	///
	/// After this method returns, previously-scheduled callbacks for `id` **must not** run.
	///
	/// # See also
	///
	/// [`SignalRuntimeRef::purge`]
	fn stop(&self, id: Self::Symbol);

	/// Executes `f` while recording dependencies for `id`, updating the recorded dependencies for `id` to the new set.
	///
	/// This process **may** cause subscription notification callbacks to be called.  
	/// This **may or may not** happen before this method returns.
	///
	/// # Logic
	///
	/// //TODO: Say that unsubscribe notifications from this **should** apply after the unsubscribing dependent has been removed (so that it won't be marked stale).
	///
	/// # Panics
	///
	/// This function **may** panic unless called between the start of [`.start`](`SignalRuntimeRef::start`) and [`.stop`](`SignalRuntimeRef::stop`) for `id`.
	///
	/// # See also
	///
	/// [`SignalRuntimeRef::purge`]
	fn update_dependency_set<T>(&self, id: Self::Symbol, f: impl FnOnce() -> T) -> T;

	/// Enables or disables the inherent subscription of `id`.
	///
	/// An inherent subscription is one that is active regardless of dependents.
	///
	/// **Idempotent** aside from the return value.  
	/// **Returns** whether there was a change in the inherent subscription.
	///
	/// # Logic
	///
	/// If the [`CallbackTable::on_subscribed_change`] returns [`Update::Propagate`],
	/// that **should** still cause refreshes of the unsubscribing dependencies (except
	/// for dependencies that have in fact been removed). This ensures that e.g. reference-
	/// counted resources can be freed appropriately. Such refreshes **may** be deferred.
	///
	/// This function **must** be callable at any time with any valid `id`.
	///
	/// # See also
	///
	/// [`SignalRuntimeRef::purge`]
	fn set_subscription(&self, id: Self::Symbol, enabled: bool) -> bool;

	/// Submits `f` to run exclusively for `id` outside of recording dependencies.
	///
	/// The runtime **should** run `f` eventually, but **may** cancel it in response to
	/// a [`.stop(id)`](`SignalRuntimeRef::stop`) call with the same `id``.
	///
	/// # Panics
	///
	/// This function **may** panic unless called between [`.start`](`SignalRuntimeRef::start`) and [`.stop`](`SignalRuntimeRef::stop`) for `id`.
	///
	/// # Safety
	///
	/// `f` **must** be dropped or consumed before the next matching [`.stop(id)`](`SignalRuntimeRef::stop`) call returns.
	fn update_or_enqueue(&self, id: Self::Symbol, f: impl 'static + Send + FnOnce() -> Propagation);

	/// **Iff polled**, submits `f` to run exclusively for `id` outside of recording dependencies.
	///
	/// The runtime **should** run `f` eventually, but **may** instead cancel and return it in response to
	/// a [`.stop(id)`](`SignalRuntimeRef::stop`) call with the same `id``.
	///
	/// # Logic
	///
	/// Calling [`.stop(id)`](`SignalRuntimeRef::stop`) with matching `id` **should** cancel the update and return the [`Err`] variant.
	///
	/// # Safety
	///
	/// `f` **must not** be dropped or run after the next matching [`.stop(id)`](`SignalRuntimeRef::stop`) call returns.  
	/// `f` **must not** be dropped or run after the [`Future`] returned by this function is dropped.
	fn update_async<T: Send, F: Send + FnOnce() -> (T, Propagation)>(
		&self,
		id: Self::Symbol,
		f: F,
	) -> impl Send + Future<Output = Result<T, F>>;

	/// Runs `f` exclusively for `id` outside of recording dependencies.
	///
	/// # Threading
	///
	/// This function **may** deadlock when called in any other exclusivity context.  
	/// (Runtimes **may** limit situations where this can occur in their documentation.)
	///
	/// # Panics
	///
	/// This function **may** panic when called in any other exclusivity context.  
	/// (Runtimes **may** limit situations where this can occur in their documentation.)
	///
	/// # Safety
	///
	/// `f` **must** be consumed before this method returns.
	fn update_blocking<T>(&self, id: Self::Symbol, f: impl FnOnce() -> (T, Propagation)) -> T;

	/// Recursively marks dependencies of `id` as stale.
	///
	/// Iff a dependency is currently subscribed, whether inherently or because of a
	/// transitive dependency, it is first updated to determine whether to propagate
	/// staleness through it, removing its stale-flag.
	fn propagate_from(&self, id: Self::Symbol);

	/// Runs `f` exempted from any outer dependency recordings.
	fn run_detached<T>(&self, f: impl FnOnce() -> T) -> T;

	/// # Safety
	///
	/// Iff `id` is stale, its staleness **must** be cleared by running its
	/// [`update`][`CallbackTable::update`] callback before this method returns.
	fn refresh(&self, id: Self::Symbol);

	/// Removes callbacks, dependency relations (in either direction) associated with `id`.
	///
	/// # Logic
	///
	/// This method **should** be called last when ceasing use of a particular `id`.  
	/// The runtime **may** indefinitely hold onto resources associated with `id` if this
	/// method isn't called.
	///
	/// The runtime **must** process resulting subscription changes appropriately. This
	/// includes notifying `id` of the subscription change from its inherent subscription
	/// being removed, where applicable.  
	/// The runtime **must not** indefinitely hold onto resources associated with `id`
	/// after this method returns.
	///
	/// The caller **may** reuse `id` later on as if fresh.
	///
	/// # Safety
	///
	/// After this method returns, previously-scheduled callbacks for `id` **must not** run.
	fn purge(&self, id: Self::Symbol);
}

struct ASignalRuntime {
	source_counter: AtomicU64,
	critical_mutex: ReentrantMutex<RefCell<ASignalRuntime_>>,
}

unsafe impl Sync for ASignalRuntime {}

struct ASignalRuntime_ {
	context_stack: Vec<Option<(ASymbol, BTreeSet<ASymbol>)>>,
	callbacks: BTreeMap<ASymbol, (*const CallbackTable<(), ACallbackTableTypes>, *const ())>,
	///FIXME: This is not-at-all a fair queue.
	update_queue: BTreeMap<ASymbol, VecDeque<Box<dyn 'static + Send + FnOnce() -> Propagation>>>,
	stale_queue: BTreeSet<ASymbol>,
	interdependencies: Interdependencies,
}

struct Interdependencies {
	/// Note: While a symbol is flagged as subscribed explicitly,
	///       it is present as its own subscriber here (by not in `all_by_dependency`!).
	/// FIXME: This could store subscriber counts instead.
	subscribers_by_dependency: BTreeMap<ASymbol, BTreeSet<ASymbol>>,
	all_by_dependent: BTreeMap<ASymbol, BTreeSet<ASymbol>>,
	all_by_dependency: BTreeMap<ASymbol, BTreeSet<ASymbol>>,
}

impl Interdependencies {
	pub(crate) const fn new() -> Self {
		Self {
			subscribers_by_dependency: BTreeMap::new(),
			all_by_dependent: BTreeMap::new(),
			all_by_dependency: BTreeMap::new(),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct ASymbol(NonZeroU64);

impl ASignalRuntime {
	const fn new() -> Self {
		Self {
			source_counter: AtomicU64::new(0),
			critical_mutex: ReentrantMutex::new(RefCell::new(ASignalRuntime_ {
				context_stack: Vec::new(),
				callbacks: BTreeMap::new(),
				update_queue: BTreeMap::new(),
				stale_queue: BTreeSet::new(),
				interdependencies: Interdependencies::new(),
			})),
		}
	}

	fn peek_stale<'a>(
		&self,
		mut borrow: RefMut<'a, ASignalRuntime_>,
	) -> (Option<ASymbol>, RefMut<'a, ASignalRuntime_>) {
		//FIXME: This is very inefficient!

		(
			borrow.stale_queue.iter().copied().find(|next| {
				!borrow
					.interdependencies
					.subscribers_by_dependency
					.get(&next)
					.expect("unreachable")
					.is_empty()
			}),
			borrow,
		)
	}

	fn pop_stale<'a>(
		&self,
		mut borrow: RefMut<'a, ASignalRuntime_>,
	) -> (Option<ASymbol>, RefMut<'a, ASignalRuntime_>) {
		//FIXME: This is very inefficient! Stale-marking propagates only forwards, so one step up in the call graph, a cursor can be used.
		if let Some(next) = borrow.stale_queue.iter().copied().find(|next| {
			borrow
				.interdependencies
				.subscribers_by_dependency
				.get(next)
				.is_some_and(|subs| !subs.is_empty())
		}) {
			assert!(borrow.stale_queue.remove(&next));
			(Some(next), borrow)
		} else {
			(None, borrow)
		}
	}

	fn subscribe_to_with<'a>(
		&self,
		dependency: ASymbol,
		dependent: ASymbol,
		lock: &'a ReentrantMutexGuard<'a, RefCell<ASignalRuntime_>>,
		mut borrow: RefMut<'a, ASignalRuntime_>,
	) -> RefMut<'a, ASignalRuntime_> {
		let subscribers = borrow
			.interdependencies
			.subscribers_by_dependency
			.entry(dependency)
			.or_default();
		if subscribers.insert(dependent) && subscribers.len() == 1 {
			// First subscriber, so propagate upwards and then call the handler!

			for transitive_dependency in borrow
				.interdependencies
				.all_by_dependent
				.entry(dependency)
				.or_default()
				.iter()
				.copied()
				.collect::<Vec<_>>()
			{
				borrow = self.subscribe_to_with(transitive_dependency, dependency, lock, borrow);
			}

			if let Some(&(callback_table, data)) = borrow.callbacks.get(&dependency) {
				unsafe {
					if let CallbackTable {
						on_subscribed_change: Some(on_subscribed_change),
						..
					} = *callback_table
					{
						// Note: Subscribed status change handlers *may* see stale values!
						// I think simpler/deduplicated propagation is likely worth that tradeoff.

						drop(borrow);
						self.run_detached(|| match on_subscribed_change(data, true) {
							Propagation::Halt => (),
							// Important: That this is within `run_detached` defers the refresh.
							// The entry point will refresh all pending (queued updates + stale subscribed)
							// in one go by calling `process_pending`.
							Propagation::Propagate => self.propagate_from(dependency),
						});
						return (**lock).borrow_mut();
					}
				}
			}
		}
		borrow
	}

	fn process_pending<'a>(
		&self,
		lock: &'a ReentrantMutexGuard<'a, RefCell<ASignalRuntime_>>,
		mut borrow: RefMut<'a, ASignalRuntime_>,
	) -> RefMut<'a, ASignalRuntime_> {
		if !borrow.context_stack.is_empty() {
			return borrow;
		}

		loop {
			while let Some((symbol, update)) = {
				let next_update;
				(next_update, borrow) = self.next_update(lock, borrow);
				next_update
			} {
				// Detach without recursion.
				borrow.context_stack.push(None);
				drop(borrow);
				let r = catch_unwind(AssertUnwindSafe(update));
				borrow = (**lock).borrow_mut();
				if let Ok(Propagation::Propagate) = &r {
					// Must run with something on `context_stack` to avoid recursion.
					self.propagate_from(symbol);
				}
				assert_eq!(borrow.context_stack.pop(), Some(None));
				if let Err(p) = r {
					resume_unwind(p)
				}
			}

			let stale;
			(stale, borrow) = self.pop_stale(borrow);
			if let Some(stale) = stale {
				self.refresh(stale);
			} else {
				break;
			}
		}

		borrow
	}

	fn next_update<'a>(
		&self,
		_lock: &'a ReentrantMutexGuard<'a, RefCell<ASignalRuntime_>>,
		mut borrow: RefMut<'a, ASignalRuntime_>,
	) -> (
		Option<(ASymbol, Box<dyn 'static + Send + FnOnce() -> Propagation>)>,
		RefMut<'a, ASignalRuntime_>,
	) {
		while let Some(mut first_group) = borrow.update_queue.first_entry() {
			if let Some(update) = first_group.get_mut().pop_front() {
				return (Some((*first_group.key(), update)), borrow);
			} else {
				drop(first_group.remove())
			}
		}
		(None, borrow)
	}
}

enum ACallbackTableTypes {}
impl CallbackTableTypes for ACallbackTableTypes {
	type SubscribedStatus = bool;
}

unsafe impl SignalRuntimeRef for &ASignalRuntime {
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

	fn record_dependency(&self, id: Self::Symbol) {
		let lock = self.critical_mutex.lock();
		let mut borrow = (*lock).borrow_mut();
		if let Some(Some((ref context_id, recorded_dependencies))) =
			&mut borrow.context_stack.last_mut()
		{
			let context_id = *context_id;

			if id >= context_id {
				panic!("Tried to depend on later-created signal. To prevent loops, this isn't possible for now.");
			}
			recorded_dependencies.insert(id);

			if !borrow
				.interdependencies
				.subscribers_by_dependency
				.entry(context_id)
				.or_default()
				.is_empty()
			{
				// It's not necessary to check if the dependency is actually new here,
				// as `subscribe_to_with` debounces automatically.

				// The subscription happens before dependency wiring.
				// This is important to avoid infinite recursion!
				borrow = self.subscribe_to_with(id, context_id, &lock, borrow);
			}

			let added_a = borrow
				.interdependencies
				.all_by_dependency
				.entry(id)
				.or_default()
				.insert(context_id);
			let added_b = borrow
				.interdependencies
				.all_by_dependent
				.entry(context_id)
				.or_default()
				.insert(id);
			debug_assert_eq!(added_a, added_b);
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
		let mut borrow = (*lock).borrow_mut();
		todo!();
		self.process_pending(&lock, borrow);
		todo!()
	}

	fn stop(&self, id: Self::Symbol) {
		let lock = self.critical_mutex.lock();
		let mut borrow = (*lock).borrow_mut();
		todo!()
	}

	fn update_dependency_set<T>(&self, id: Self::Symbol, f: impl FnOnce() -> T) -> T {
		let lock = self.critical_mutex.lock();
		let mut borrow = (*lock).borrow_mut();
		todo!();
		self.process_pending(&lock, borrow);
		todo!()
	}

	fn set_subscription(&self, id: Self::Symbol, enabled: bool) -> bool {
		let lock = self.critical_mutex.lock();
		let mut borrow = (*lock).borrow_mut();
		todo!();
		self.process_pending(&lock, borrow);
		todo!()
	}

	fn update_or_enqueue(
		&self,
		id: Self::Symbol,
		f: impl 'static + Send + FnOnce() -> Propagation,
	) {
		let lock = self.critical_mutex.lock();
		let mut borrow = (*lock).borrow_mut();
		todo!();
		self.process_pending(&lock, borrow);
		todo!()
	}

	async fn update_async<T: Send, F: Send + FnOnce() -> (T, Propagation)>(
		&self,
		id: Self::Symbol,
		f: F,
	) -> Result<T, F> {
		let f = Arc::new(Mutex::new(Some(f)));
		let _f_guard = guard(Arc::clone(&f), |f| drop(f.lock().unwrap().take()));

		let once = Arc::new(OnceCell::<Mutex<Option<Result<T, Option<F>>>>>::new());
		let update = Box::new({
			let weak: Weak<_> = Arc::downgrade(&once);
			let guard = {
				let weak = weak.clone();
				guard(f, move |f| {
					if let Some(once) = weak.upgrade() {
						once.set_blocking(
							Some(Err(f.lock().expect("unreachable").borrow_mut().take())).into(),
						)
						.map_err(|_| ())
						.expect("unreachable");
					}
				})
			};
			move || {
				// Allow (rough) cancellation.
				let arc = ScopeGuard::into_inner(guard);
				let mut f_guard = arc.lock().expect("unreachable");
				if let (Some(once), Some(f)) = (weak.upgrade(), f_guard.borrow_mut().take()) {
					let (t, update) = f();
					once.set_blocking(Some(Ok(t)).into())
						.map_err(|_| ())
						.expect("unreachable");
					update
				} else {
					Propagation::Halt
				}
			}
		});

		self.update_or_enqueue(id, unsafe {
			//SAFETY: This function never handles `F` or `T` after `_f_guard` drops.
			mem::transmute::<
				Box<dyn '_ + Send + FnOnce() -> Propagation>,
				Box<dyn 'static + Send + FnOnce() -> Propagation>,
			>(update)
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
			Some(Err(f)) => {
				return Err(f.expect("`_f_guard` didn't destroy `f` yet at this point."))
			}
			None => unreachable!(),
		};

		// Wait again so that propagation also completes first.
		let once = Arc::new(OnceCell::<()>::new());
		self.update_or_enqueue(id, {
			let guard = guard(Arc::downgrade(&once), |c| {
				if let Some(c) = c.upgrade() {
					c.set_blocking(()).expect("unreachable");
				}
			});
			move || {
				drop(guard);
				Propagation::Halt
			}
		});

		once.wait().await;

		Ok(t)
	}

	fn update_blocking<T>(&self, id: Self::Symbol, f: impl FnOnce() -> (T, Propagation)) -> T {
		// This is indirected because the nested function's text size may be relatively large.
		//BLOCKED: Avoid the heap allocation once the `Allocator` API is stabilised.

		fn update_blocking<T>(
			this: &ASignalRuntime,
			id: ASymbol,
			f: Box<dyn '_ + FnOnce() -> (T, Propagation)>,
		) -> T {
			let lock = this.critical_mutex.lock();
			let borrow = (*lock).borrow_mut();

			let (stale, borrow) = this.peek_stale(borrow);
			let has_stale = stale.is_some();

			if !(borrow.context_stack.is_empty() && !has_stale) {
				panic!("Called `update_blocking` (via `change_blocking` or `replace_blocking`?) while propagating another update. This would deadlock with a better queue.");
			}

			let (t, update) = f();
			drop(borrow);
			match update {
				Propagation::Propagate => this.propagate_from(id),
				Propagation::Halt => (),
			}
			t
		}
		update_blocking(self, id, Box::new(f))
	}

	fn propagate_from(&self, id: Self::Symbol) {
		let lock = self.critical_mutex.lock();
		let mut borrow = (*lock).borrow_mut();
		todo!();
		self.process_pending(&lock, borrow);
		todo!()
	}

	fn run_detached<T>(&self, f: impl FnOnce() -> T) -> T {
		let lock = self.critical_mutex.lock();
		let mut borrow = (*lock).borrow_mut();
		borrow.context_stack.push(None);
		drop(borrow);
		let r = catch_unwind(AssertUnwindSafe(f));
		borrow = (*lock).borrow_mut();
		assert_eq!(borrow.context_stack.pop(), Some(None));
		self.process_pending(&lock, borrow);
		r.unwrap_or_else(|p| resume_unwind(p))
	}

	fn refresh(&self, id: Self::Symbol) {
		let lock = self.critical_mutex.lock();
		let mut borrow = (*lock).borrow_mut();
		todo!();
		self.process_pending(&lock, borrow);
		todo!()
	}

	fn purge(&self, id: Self::Symbol) {
		let lock = self.critical_mutex.lock();
		let mut borrow = (*lock).borrow_mut();
		todo!();
		self.process_pending(&lock, borrow);
		todo!()
	}
}

static GLOBAL_SIGNAL_RUNTIME: ASignalRuntime = ASignalRuntime::new();

/// A plain [`SignalRuntimeRef`] implementation that represents a static signal runtime.
///
/// 🚧 This implementation is currently not optimised. 🚧
///
/// # Logic
///
/// This runtime is guaranteed to have settled whenever the last borrow of it ceases, but
/// only regarding effects originating on the current thread. Effects from other threads
/// won't necessarily be visible without external synchronisation.
///
/// (This means that in addition to transiently borrowing calls, returned [`Future`]s
/// **may** cause the [`GlobalSignalRuntime`] not to settle until they are dropped.)
///
/// Otherwise, it makes no additional guarantees over those specified in [`SignalRuntimeRef`]'s documentation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlobalSignalRuntime;

/// [`SignalRuntimeRef::Symbol`] for [`GlobalSignalRuntime`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GSRSymbol(ASymbol);

impl Debug for GSRSymbol {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_tuple("GSRSymbol").field(&self.0 .0).finish()
	}
}

/// [`SignalRuntimeRef::CallbackTableTypes`] for [`GlobalSignalRuntime`].
#[repr(transparent)]
pub struct GlobalCallbackTableTypes(ACallbackTableTypes);
impl CallbackTableTypes for GlobalCallbackTableTypes {
	//SAFETY: Everything here must be the same as for `ACallbackTableTypes`!
	type SubscribedStatus = bool;
}

unsafe impl SignalRuntimeRef for GlobalSignalRuntime {
	type Symbol = GSRSymbol;
	type CallbackTableTypes = GlobalCallbackTableTypes;

	fn next_id(&self) -> GSRSymbol {
		GSRSymbol((&GLOBAL_SIGNAL_RUNTIME).next_id())
	}

	fn record_dependency(&self, id: Self::Symbol) {
		(&GLOBAL_SIGNAL_RUNTIME).record_dependency(id.0)
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

	fn stop(&self, id: Self::Symbol) {
		(&GLOBAL_SIGNAL_RUNTIME).stop(id.0)
	}

	fn update_dependency_set<T>(&self, id: Self::Symbol, f: impl FnOnce() -> T) -> T {
		(&GLOBAL_SIGNAL_RUNTIME).update_dependency_set(id.0, f)
	}

	fn set_subscription(&self, id: Self::Symbol, enabled: bool) -> bool {
		(&GLOBAL_SIGNAL_RUNTIME).set_subscription(id.0, enabled)
	}

	fn update_or_enqueue(
		&self,
		id: Self::Symbol,
		f: impl 'static + Send + FnOnce() -> Propagation,
	) {
		(&GLOBAL_SIGNAL_RUNTIME).update_or_enqueue(id.0, f)
	}

	async fn update_async<T: Send, F: Send + FnOnce() -> (T, Propagation)>(
		&self,
		id: Self::Symbol,
		f: F,
	) -> Result<T, F> {
		(&GLOBAL_SIGNAL_RUNTIME).update_async(id.0, f).await
	}

	fn update_blocking<T>(&self, id: Self::Symbol, f: impl FnOnce() -> (T, Propagation)) -> T {
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

	fn purge(&self, id: Self::Symbol) {
		(&GLOBAL_SIGNAL_RUNTIME).purge(id.0)
	}
}

/// The `unsafe` at-runtime version of [`Callbacks`](`crate::raw::Callbacks`),
/// mainly for use between [`RawSignal`](`crate::raw::RawSignal`) and [`SignalRuntimeRef`].
#[repr(C)]
#[non_exhaustive]
pub struct CallbackTable<T: ?Sized, CTT: ?Sized + CallbackTableTypes> {
	/// An "update" callback used to refresh stale signals.
	///
	/// Signals that are not currently subscribed don't auto-refresh and **may** remain stale for extended periods of time.
	///
	/// # Safety
	///
	/// This **must** be called by the runtime at most with the appropriate `callback_data` pointer introduced alongside the function pointer,
	/// and **must not** be called concurrently within the group of callbacks associated with one `id`.
	pub update: Option<unsafe fn(*const T) -> Propagation>,
	/// An "on subscribed change" callback used to notify a signal of a change in its subscribed-state.
	///
	/// This is separate from the automatic refresh applied to stale signals that become subscribed to.
	///
	/// # Safety
	///
	/// This **must** be called by the runtime at most with the appropriate `callback_data` pointer introduced alongside the function pointer,
	/// and **must not** be called concurrently within the group of callbacks associated with one `id`.
	///
	/// # Logic
	///
	/// The runtime **must** consider transitive subscriptions.  
	/// The runtime **must** consider a signal's own inherent subscription.  
	/// The runtime **must not** run this function while recording dependencies (but may start a nested recording in response to the callback).
	pub on_subscribed_change:
		Option<unsafe fn(*const T, status: CTT::SubscribedStatus) -> Propagation>,
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

/// Describes types appearing in callback signatures for a particular [`SignalRuntimeRef`] implementation.
pub trait CallbackTableTypes: 'static {
	/// A status indicating "how subscribed" a signal now is.
	///
	/// [`GlobalSignalRuntime`] notifies only for the first and removal of the last subscription for each signal,
	/// so it uses a [`bool`], but other runtimes may notify with the direct or total subscriber count or a more complex measure.
	type SubscribedStatus;
}

impl<T: ?Sized, CTT: ?Sized + CallbackTableTypes> CallbackTable<T, CTT> {
	/// "Type-erases" the pointed-to callback table against the data type `T` by replacing it with `()`.
	///
	/// Note that the callback functions still may only be called using the originally correct data pointer(s).
	pub fn into_erased_ptr(this: *const Self) -> *const CallbackTable<(), CTT> {
		unsafe { mem::transmute(this) }
	}

	/// "Type-erases" the pointed-to callback table against the data type `T` by replacing it with `()`.
	///
	/// Note that the callback functions still may only be called using the originally correct data pointer(s).
	pub fn into_erased(self) -> CallbackTable<(), CTT> {
		unsafe { mem::transmute(self) }
	}
}

/// A return value used by [`CallbackTable`]/[`Callbacks`](`crate::raw::Callbacks`) callbacks
/// to indicate whether to flag dependent signals as stale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[must_use = "The runtime should propagate notifications to dependents only when requested."]
pub enum Propagation {
	/// Mark at least directly dependent signals, and possibly refresh them.
	Propagate,
	/// Do not mark dependent signals as stale, except through other (parallel) dependency relationships.
	Halt,
}
