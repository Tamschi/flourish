use std::{
	borrow::{Borrow, BorrowMut as _},
	cell::{RefCell, RefMut},
	collections::{BTreeMap, BTreeSet, VecDeque},
	fmt::{self, Debug, Formatter},
	mem,
	sync::{atomic::Ordering, Arc, Mutex},
};

use core::sync::atomic::AtomicU64;
use parking_lot::{ReentrantMutex, ReentrantMutexGuard};
use scopeguard::{guard, ScopeGuard};
use unwind_safe::try_eval;

use super::{private, ACallbackTableTypes, ASymbol, CallbackTable, Propagation, SignalsRuntimeRef};

#[derive(Debug)]
pub(crate) struct ASignalsRuntime {
	source_counter: AtomicU64,
	critical_mutex: ReentrantMutex<RefCell<ASignalsRuntime_>>,
}

unsafe impl Sync for ASignalsRuntime {}

struct ASignalsRuntime_ {
	context_stack: Vec<Option<(ASymbol, BTreeSet<ASymbol>)>>,
	callbacks: BTreeMap<ASymbol, (*const CallbackTable<(), ACallbackTableTypes>, *const ())>,
	///FIXME: This is not-at-all a fair queue.
	update_queue: BTreeMap<ASymbol, VecDeque<Box<dyn 'static + Send + FnOnce() -> Propagation>>>,
	stale_queue: BTreeSet<Stale>,
	interdependencies: Interdependencies,
}

#[derive(Debug, Clone, Copy, Eq)]
struct Stale {
	symbol: ASymbol,
	flush: bool,
}

impl Borrow<ASymbol> for Stale {
	fn borrow(&self) -> &ASymbol {
		&self.symbol
	}
}

impl PartialOrd for Stale {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl Ord for Stale {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.symbol.cmp(&other.symbol)
	}
}

impl PartialEq for Stale {
	fn eq(&self, other: &Self) -> bool {
		self.symbol == other.symbol
	}
}

impl Debug for ASignalsRuntime_ {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		f.debug_struct("ASignalsRuntime_")
			.field("context_stack", &self.context_stack)
			.field("callbacks", &self.callbacks)
			.field("update_queue", &self.update_queue.keys())
			.field("stale_queue", &self.stale_queue)
			//FIXME: This could be a lot nicer, for example by printing a dependency graph (if a feature to do so is enabled).
			.field("interdependencies", &self.interdependencies)
			.finish()
	}
}

#[derive(Debug)]
struct Interdependencies {
	/// Note: While a symbol is flagged as subscribed explicitly,
	///       it is present as its own subscriber here (by not in `all_by_dependency`!).
	/// FIXME: This could store subscriber counts instead.
	subscribers_by_dependency: BTreeMap<ASymbol, Subscribers>,
	all_by_dependent: BTreeMap<ASymbol, BTreeSet<ASymbol>>,
	all_by_dependency: BTreeMap<ASymbol, BTreeSet<ASymbol>>,
}

#[derive(Debug, Default)]
struct Subscribers {
	intrinsic: u64,
	extrinsic: BTreeSet<ASymbol>,
}

impl Subscribers {
	fn is_empty(&self) -> bool {
		self.intrinsic == 0 && self.extrinsic.is_empty()
	}

	fn total(&self) -> u64 {
		self.intrinsic
			.checked_add(
				self.extrinsic
					.len()
					.try_into()
					.expect("too many extrinsic subscriptions"),
			)
			.expect("too many subscriptions")
	}
}

impl Interdependencies {
	const fn new() -> Self {
		Self {
			subscribers_by_dependency: BTreeMap::new(),
			all_by_dependent: BTreeMap::new(),
			all_by_dependency: BTreeMap::new(),
		}
	}
}

impl ASignalsRuntime {
	pub(crate) const fn new() -> Self {
		Self {
			source_counter: AtomicU64::new(0),
			critical_mutex: ReentrantMutex::new(RefCell::new(ASignalsRuntime_ {
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
		borrow: RefMut<'a, ASignalsRuntime_>,
	) -> (Option<Stale>, RefMut<'a, ASignalsRuntime_>) {
		//FIXME: This is very inefficient!

		(
			borrow
				.stale_queue
				.iter()
				.copied()
				.find(|&Stale { ref symbol, flush }| {
					flush
						|| !borrow
							.interdependencies
							.subscribers_by_dependency
							.get(symbol)
							.expect("unreachable")
							.is_empty()
				}),
			borrow,
		)
	}

	fn subscribe_to_with<'a>(
		&self,
		dependency: ASymbol,
		dependent: ASymbol,
		lock: &'a ReentrantMutexGuard<'a, RefCell<ASignalsRuntime_>>,
		mut borrow: RefMut<'a, ASignalsRuntime_>,
	) -> RefMut<'a, ASignalsRuntime_> {
		let subscribers = borrow
			.interdependencies
			.subscribers_by_dependency
			.entry(dependency)
			.or_default();

		if if dependency == dependent {
			subscribers.intrinsic = subscribers
				.intrinsic
				.checked_add(1)
				.expect("The intrinsic subscription count became too high.");
			true
		} else {
			subscribers.extrinsic.insert(dependent)
		} && subscribers.total() == 1
		{
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

						// Important guard frame against `stop` and `purge`!
						borrow
							.context_stack
							.push(Some((dependency, BTreeSet::new())));
						borrow.context_stack.push(None);
						drop(borrow);
						let propagation =
							try_eval(|| on_subscribed_change(data, true)).finally(|()| {
								let mut borrow = (**lock).borrow_mut();
								assert_eq!(borrow.context_stack.pop(), Some(None));
								assert_eq!(
									borrow.context_stack.pop(),
									Some(Some((dependency, BTreeSet::new())))
								);
							});
						borrow = (**lock).borrow_mut();
						borrow = match propagation {
							Propagation::Halt => borrow,
							Propagation::Propagate => {
								self.mark_dependencies_stale(dependency, &lock, borrow, false)
							}
							Propagation::FlushOut => {
								self.mark_dependencies_stale(dependency, &lock, borrow, true)
							}
						}
					}
				}
			}
		}
		borrow
	}

	fn unsubscribe_from_with<'a>(
		&self,
		dependency: ASymbol,
		dependent: ASymbol,
		lock: &'a ReentrantMutexGuard<'a, RefCell<ASignalsRuntime_>>,
		mut borrow: RefMut<'a, ASignalsRuntime_>,
	) -> RefMut<'a, ASignalsRuntime_> {
		let subscribers = borrow
			.interdependencies
			.subscribers_by_dependency
			.entry(dependency)
			.or_default();
		if if dependency == dependent {
			subscribers.intrinsic = subscribers
				.intrinsic
				.checked_sub(1)
				.expect("Tried to decrement intrinsic subscriber count below 0.");
			true
		} else {
			subscribers.extrinsic.remove(&dependent)
		} && subscribers.total() == 0
		{
			// Removed last subscriber, so propagate upwards and then call the handler!

			for transitive_dependency in borrow
				.interdependencies
				.all_by_dependent
				.entry(dependency)
				.or_default()
				.iter()
				.copied()
				.collect::<Vec<_>>()
			{
				borrow =
					self.unsubscribe_from_with(transitive_dependency, dependency, lock, borrow);
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

						// Important guard frame against `stop` and `purge`!
						borrow
							.context_stack
							.push(Some((dependency, BTreeSet::new())));
						borrow.context_stack.push(None);
						drop(borrow);
						let propagation =
							try_eval(|| on_subscribed_change(data, false)).finally(|()| {
								let mut borrow = (**lock).borrow_mut();
								assert_eq!(borrow.context_stack.pop(), Some(None));
								assert_eq!(
									borrow.context_stack.pop(),
									Some(Some((dependency, BTreeSet::new())))
								);
							});
						borrow = (**lock).borrow_mut();
						borrow = match propagation {
							Propagation::Halt => borrow,
							Propagation::Propagate => {
								self.mark_dependencies_stale(dependency, &lock, borrow, false)
							}
							Propagation::FlushOut => {
								self.mark_dependencies_stale(dependency, &lock, borrow, true)
							}
						}
					}
				}
			}
		}

		borrow
	}

	fn process_pending<'a>(
		&self,
		lock: &'a ReentrantMutexGuard<'a, RefCell<ASignalsRuntime_>>,
		mut borrow: RefMut<'a, ASignalsRuntime_>,
	) -> RefMut<'a, ASignalsRuntime_> {
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
				let propagation = try_eval(|| {
					borrow.context_stack.push(None);
					drop(borrow);
					update()
				})
				.finally(|()| {
					let mut borrow = (**lock).borrow_mut();
					assert_eq!(borrow.context_stack.pop(), Some(None));
				});
				borrow = (**lock).borrow_mut();
				match propagation {
					Propagation::Propagate => {
						borrow = self.mark_dependencies_stale(symbol, &lock, borrow, false)
					}
					Propagation::Halt => (),
					Propagation::FlushOut => {
						borrow = self.mark_dependencies_stale(symbol, &lock, borrow, true)
					}
				}
			}

			let stale;
			(stale, borrow) = self.peek_stale(borrow);
			if let Some(Stale { symbol, flush: _ }) = stale {
				try_eval(|| {
					borrow.context_stack.push(None);
					drop(borrow);
					self.refresh(symbol)
				})
				.finally(|()| {
					let mut borrow = (**lock).borrow_mut();
					assert_eq!(borrow.context_stack.pop(), Some(None));
				});
				borrow = (**lock).borrow_mut();
			} else {
				break;
			}
		}

		borrow
	}

	fn next_update<'a>(
		&self,
		_lock: &'a ReentrantMutexGuard<'a, RefCell<ASignalsRuntime_>>,
		mut borrow: RefMut<'a, ASignalsRuntime_>,
	) -> (
		Option<(ASymbol, Box<dyn 'static + Send + FnOnce() -> Propagation>)>,
		RefMut<'a, ASignalsRuntime_>,
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

	fn mark_dependencies_stale<'a>(
		&self,
		id: ASymbol,
		lock: &'a ReentrantMutexGuard<'a, RefCell<ASignalsRuntime_>>,
		mut borrow: RefMut<'a, ASignalsRuntime_>,
		flush: bool,
	) -> RefMut<'a, ASignalsRuntime_> {
		let dependents = borrow
			.interdependencies
			.all_by_dependency
			.entry(id)
			.or_default()
			.iter()
			.copied()
			.collect::<Vec<_>>();

		if flush {
			for symbol in dependents {
				if borrow
					.stale_queue
					.replace(Stale { symbol, flush })
					.is_none() && borrow
					.interdependencies
					.subscribers_by_dependency
					.entry(symbol)
					.or_default()
					.is_empty()
				{
					// The dependency wasn't marked stale yet and also won't update, so recurse.
					// Note that flushing is propagated during the refresh instead!
					borrow = self.mark_dependencies_stale(symbol, lock, borrow, false);
				}
			}
		} else {
			for symbol in dependents {
				if borrow.stale_queue.insert(Stale { symbol, flush })
					&& borrow
						.interdependencies
						.subscribers_by_dependency
						.entry(symbol)
						.or_default()
						.is_empty()
				{
					// The dependency wasn't marked stale yet and also won't update, so recurse.
					borrow = self.mark_dependencies_stale(symbol, lock, borrow, false);
				}
			}
		}
		borrow
	}

	fn shrink_dependencies<'a>(
		&self,
		id: ASymbol,
		recorded_dependencies: BTreeSet<ASymbol>,
		lock: &'a ReentrantMutexGuard<'a, RefCell<ASignalsRuntime_>>,
		mut borrow: RefMut<'a, ASignalsRuntime_>,
	) -> RefMut<'a, ASignalsRuntime_> {
		let prior_dependencies = borrow
			.interdependencies
			.all_by_dependent
			.entry(id)
			.or_default();

		assert!(recorded_dependencies.is_subset(prior_dependencies));

		let removed_dependencies = &*prior_dependencies - &recorded_dependencies;
		drop(
			borrow
				.interdependencies
				.all_by_dependent
				.insert(id, recorded_dependencies),
		);

		for removed_dependency in &removed_dependencies {
			assert!(borrow
				.interdependencies
				.all_by_dependency
				.get_mut(removed_dependency)
				.expect("These lists should always be symmetrical at rest.")
				.remove(&id))
		}

		let is_subscribed = borrow
			.interdependencies
			.subscribers_by_dependency
			.get(&id)
			.is_some_and(|subs| !subs.is_empty());
		if is_subscribed {
			for removed_dependency in removed_dependencies {
				borrow = self.unsubscribe_from_with(removed_dependency, id, lock, borrow)
			}
		}

		borrow
	}
}

unsafe impl SignalsRuntimeRef for &ASignalsRuntime {
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
				// as `subscribe_to_with` filters that automatically.

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

		self.process_pending(&lock, borrow);
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

		if borrow.callbacks.contains_key(&id) {
			panic!("Tried to `start` `id` twice.")
		}

		let t = try_eval(|| {
			borrow.context_stack.push(Some((id, BTreeSet::new())));
			drop(borrow);
			f()
		})
		.finally(|()| {
			let mut borrow = (*lock).borrow_mut();
			let Some(Some((popped_id, recorded_dependencies))) = borrow.context_stack.pop() else {
				unreachable!()
			};
			assert_eq!(popped_id, id);

			// This is a bit of a patch-fix against double-calls when subscribing to a stale signal.
			//TODO: Instead, add the dependency after subscribing when recording it!
			borrow.stale_queue.remove(&id);
			assert_eq!(
				borrow.callbacks.insert(
					id,
					(
						CallbackTable::into_erased_ptr(callback_table),
						callback_data.cast::<()>()
					)
				),
				None
			);
			let _ = self.shrink_dependencies(id, recorded_dependencies, &lock, borrow);
		});
		borrow = (*lock).borrow_mut();

		if borrow
			.interdependencies
			.subscribers_by_dependency
			.get(&id)
			.is_some_and(|subs| !subs.is_empty())
		{
			// Subscribed, so run the callback for that.
			let propagation = try_eval(|| {
				// Important guard frame against `stop` and `purge`!
				borrow.context_stack.push(Some((id, BTreeSet::new())));
				borrow.context_stack.push(None);
				drop(borrow);
				unsafe {
					if let &CallbackTable {
						on_subscribed_change: Some(on_subscribed_change),
						..
					} = &*callback_table
					{
						let propagation = on_subscribed_change(callback_data, true);
						propagation
					} else {
						Propagation::Halt
					}
				}
			})
			.finally(|()| {
				let mut borrow = (*lock).borrow_mut();
				assert_eq!(borrow.context_stack.pop(), Some(None));
				assert_eq!(
					borrow.context_stack.pop(),
					Some(Some((id, BTreeSet::new())))
				);
			});

			borrow = (*lock).borrow_mut();
			borrow = match propagation {
				Propagation::Propagate => self.mark_dependencies_stale(id, &lock, borrow, false),
				Propagation::Halt => borrow,
				Propagation::FlushOut => self.mark_dependencies_stale(id, &lock, borrow, true),
			};
		}

		self.process_pending(&lock, borrow);
		t
	}

	fn stop(&self, id: Self::Symbol) {
		let lock = self.critical_mutex.lock();
		let mut borrow = (*lock).borrow_mut();

		if borrow
			.context_stack
			.iter()
			.flatten()
			.any(|(stack_id, _)| *stack_id == id)
		{
			panic!("Tried to stop `id` in its own context.");
		}

		borrow.callbacks.remove(&id);

		// This can unblock futures.
		// Note that this could schedule more work for `id`!
		// This method only guarantees _previous_ updates have been stopped.
		drop(borrow.update_queue.remove(&id));

		// There may have been side-effects.
		self.process_pending(&lock, borrow);
	}

	fn update_dependency_set<T>(&self, id: Self::Symbol, f: impl FnOnce() -> T) -> T {
		let lock = self.critical_mutex.lock();
		let mut borrow = (*lock).borrow_mut();

		let t = try_eval(|| {
			borrow.context_stack.push(Some((id, BTreeSet::new())));
			drop(borrow);
			f()
		})
		.finally(|()| {
			let mut borrow = (*lock).borrow_mut();
			let Some(Some((popped_id, recorded_dependencies))) = borrow.context_stack.pop() else {
				unreachable!()
			};
			assert_eq!(popped_id, id);
			let _ = self.shrink_dependencies(id, recorded_dependencies, &lock, borrow);
		});

		borrow = (*lock).borrow_mut();
		self.process_pending(&lock, borrow);
		t
	}

	fn subscribe(&self, id: Self::Symbol) {
		let lock = self.critical_mutex.lock();
		let mut borrow = (*lock).borrow_mut();

		borrow = self.subscribe_to_with(id, id, &lock, borrow);

		self.process_pending(&lock, borrow);
	}

	fn unsubscribe(&self, id: Self::Symbol) {
		let lock = self.critical_mutex.lock();
		let mut borrow = (*lock).borrow_mut();

		borrow = self.unsubscribe_from_with(id, id, &lock, borrow);

		self.process_pending(&lock, borrow);
	}

	fn update_or_enqueue(
		&self,
		id: Self::Symbol,
		f: impl 'static + Send + FnOnce() -> Propagation,
	) {
		let lock = self.critical_mutex.lock();
		let mut borrow = (*lock).borrow_mut();
		borrow
			.update_queue
			.entry(id)
			.or_default()
			.push_back(Box::new(f));
		self.process_pending(&lock, borrow);
	}

	fn update_eager<'f, T: 'f + Send, F: 'f + Send + FnOnce() -> (Propagation, T)>(
		&self,
		id: Self::Symbol,
		f: F,
	) -> Self::UpdateEager<'f, T, F> {
		let f = Arc::new(Mutex::new(Some(f)));
		let _f_guard = guard(Arc::clone(&f), |f| drop(f.lock().unwrap().take()));

		let once = Arc::new(
			async_lock::Mutex::<Mutex<Option<Result<T, Option<F>>>>>::new(Mutex::new(None)),
		);
		let setter_lock = Arc::new(Mutex::new(Some(once.try_lock_arc().expect("unreachable"))));
		let _setter_lock_guard = guard(Arc::clone(&setter_lock), |setter_lock| {
			drop(setter_lock.lock().expect("unreachable").take());
		});

		let update = Box::new({
			let setter_lock = Arc::clone(&setter_lock);
			let guard = {
				let setter_lock = Arc::clone(&setter_lock);
				guard(f, move |f| {
					if let Some(mut setter_lock) = setter_lock.lock().expect("unreachable").take() {
						*setter_lock =
							Some(Err(f.lock().expect("unreachable").borrow_mut().take())).into();
					}
				})
			};
			move || {
				// Allow (rough) cancellation.
				let arc = ScopeGuard::into_inner(guard);
				let mut f_guard = arc.lock().expect("unreachable");
				if let (Some(mut setter_lock), Some(f)) = (
					setter_lock.lock().expect("unreachable").take(),
					f_guard.borrow_mut().take(),
				) {
					let (propagation, t) = f();
					*setter_lock = Some(Ok(t)).into();
					propagation
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

		let lock = self.critical_mutex.lock();
		let borrow = (*lock).borrow_mut();
		self.process_pending(&lock, borrow);

		private::DetachedFuture(Box::pin(async move {
			match once
				.lock()
				.await
				.lock()
				.expect("unreachable")
				.borrow_mut()
				.take()
			{
				Some(Ok(t)) => return Ok(t),
				Some(Err(f)) => {
					return Err(f.expect("`_f_guard` didn't destroy `f` yet at this point."))
				}
				None => unreachable!(),
			};
		}))
	}

	type UpdateEager<'f, T: 'f, F: 'f> = private::DetachedFuture<'f, Result<T, F>>;

	fn update_blocking<T>(&self, id: Self::Symbol, f: impl FnOnce() -> (Propagation, T)) -> T {
		// This is indirected because the nested function's text size may be relatively large.
		//BLOCKED: Avoid the heap allocation once the `Allocator` API is stabilised.

		fn update_blocking<T>(
			this: &ASignalsRuntime,
			id: ASymbol,
			f: Box<dyn '_ + FnOnce() -> (Propagation, T)>,
		) -> T {
			let lock = this.critical_mutex.lock();
			let borrow = (*lock).borrow_mut();

			let (stale, mut borrow) = this.peek_stale(borrow);
			let has_stale = stale.is_some();

			if !(borrow.context_stack.is_empty() && !has_stale) {
				panic!("Called `update_blocking` (via `change_blocking` or `replace_blocking`?) while propagating another update. This would deadlock with a better queue.");
			}

			let (propagation, t) = f();
			borrow = match propagation {
				Propagation::Propagate => this.mark_dependencies_stale(id, &lock, borrow, false),
				Propagation::Halt => borrow,
				Propagation::FlushOut => this.mark_dependencies_stale(id, &lock, borrow, true),
			};
			this.process_pending(&lock, borrow);
			t
		}
		update_blocking(self, id, Box::new(f))
	}

	fn run_detached<T>(&self, f: impl FnOnce() -> T) -> T {
		let lock = self.critical_mutex.lock();
		let mut borrow = (*lock).borrow_mut();
		let t = try_eval(|| {
			borrow.context_stack.push(None);
			drop(borrow);
			f()
		})
		.finally(|()| {
			let mut borrow = (*lock).borrow_mut();
			assert_eq!(borrow.context_stack.pop(), Some(None));
		});
		borrow = (*lock).borrow_mut();
		self.process_pending(&lock, borrow);
		t
	}

	fn refresh(&self, id: Self::Symbol) {
		let lock = self.critical_mutex.lock();
		let mut borrow = (*lock).borrow_mut();
		if let Some(Stale { symbol: _, flush }) = borrow.stale_queue.take(&id) {
			if let Some(&(callback_table, data)) = borrow.callbacks.get(&id) {
				if let &CallbackTable {
					update: Some(update),
					..
				} = unsafe { &*callback_table }
				{
					let propagation = try_eval(|| {
						borrow.context_stack.push(None);
						drop(borrow);
						self.update_dependency_set(id, || unsafe { update(data) })
					})
					.finally(|()| {
						let mut borrow = (*lock).borrow_mut();
						assert_eq!(borrow.context_stack.pop(), Some(None));
					});
					borrow = (*lock).borrow_mut();
					match propagation {
						Propagation::Propagate => {
							borrow = self.mark_dependencies_stale(id, &lock, borrow, flush)
						}
						Propagation::Halt => (),
						Propagation::FlushOut => {
							borrow = self.mark_dependencies_stale(id, &lock, borrow, true)
						}
					}
				} else {
					// If there's no callback, then always mark dependencies as stale!
					// (This happens with uncached signals, for example.)
					borrow = self.mark_dependencies_stale(id, &lock, borrow, flush);
				}
			} else {
				// If there's no callback, then always mark dependencies as stale!
				// (This happens with uncached signals, for example.)
				borrow = self.mark_dependencies_stale(id, &lock, borrow, flush);
			}
		}
		self.process_pending(&lock, borrow);
	}

	fn purge(&self, id: Self::Symbol) {
		let lock = self.critical_mutex.lock();
		let mut borrow = (*lock).borrow_mut();

		if borrow
			.context_stack
			.iter()
			.flatten()
			.any(|(stack_id, _)| *stack_id == id)
		{
			panic!("Tried to purge `id` in its own context.");
		}

		borrow = self.shrink_dependencies(id, BTreeSet::new(), &lock, borrow);
		for dependent in borrow
			.interdependencies
			.all_by_dependency
			.entry(id)
			.or_default()
			.iter()
			.copied()
			.collect::<Vec<_>>()
		{
			borrow = self.shrink_dependencies(
				dependent,
				&*borrow
					.interdependencies
					.all_by_dependent
					.entry(dependent)
					.or_default() - &[id].into(),
				&lock,
				borrow,
			);
		}

		while borrow
			.interdependencies
			.subscribers_by_dependency
			.entry(id)
			.or_default()
			.intrinsic
			> 0
		{
			borrow = self.unsubscribe_from_with(id, id, &lock, borrow);
		}

		borrow.callbacks.remove(&id);

		// This can unblock futures.
		// Note that this could schedule more work for `id`!
		// This method only guarantees _previous_ updates have been stopped.
		drop(borrow.update_queue.remove(&id));

		let interdependencies = &mut borrow.interdependencies;
		for collection in [
			&mut interdependencies.all_by_dependency,
			&mut interdependencies.all_by_dependent,
		] {
			assert!(!collection
				.remove(&id)
				.is_some_and(|linked| !linked.is_empty()))
		}

		assert!(!interdependencies
			.subscribers_by_dependency
			.remove(&id)
			.is_some_and(|subscribers| !subscribers.is_empty()));

		borrow.stale_queue.remove(&id);

		self.process_pending(&lock, borrow);
	}

	fn hint_batched_updates<T>(&self, f: impl FnOnce() -> T) -> T {
		// Ensures that the context stack is not empty while `f` runs, blocking updates.
		let lock = self.critical_mutex.lock();
		let mut borrow = (*lock).borrow_mut();
		if borrow.context_stack.is_empty() {
			let t = try_eval(|| {
				borrow.context_stack.push(None);
				drop(borrow);
				f()
			})
			.finally(|()| {
				let mut borrow = (*lock).borrow_mut();
				assert_eq!(borrow.context_stack.pop(), Some(None));
			});
			borrow = (*lock).borrow_mut();
			self.process_pending(&lock, borrow);
			t
		} else {
			drop(borrow);
			f()
		}
	}
}
