use std::{
	borrow::BorrowMut as _,
	cell::{RefCell, RefMut},
	collections::{BTreeMap, BTreeSet, VecDeque},
	fmt::Debug,
	mem,
	sync::{atomic::Ordering, Arc, Mutex, Weak},
};

use async_lock::OnceCell;
use core::{num::NonZeroU64, sync::atomic::AtomicU64};
use parking_lot::{ReentrantMutex, ReentrantMutexGuard};
use scopeguard::{guard, ScopeGuard};
use unwind_safe::try_eval;

use super::{
	private, ACallbackTableTypes, ASymbol, CallbackTable, CallbackTableTypes, Propagation,
	SignalRuntimeRef,
};

#[derive(Debug)]
pub(crate) struct ASignalRuntime {
	pub(crate) source_counter: AtomicU64,
	pub(crate) critical_mutex: ReentrantMutex<RefCell<ASignalRuntime_>>,
}

unsafe impl Sync for ASignalRuntime {}

pub(crate) struct ASignalRuntime_ {
	pub(crate) context_stack: Vec<Option<(ASymbol, BTreeSet<ASymbol>)>>,
	pub(crate) callbacks:
		BTreeMap<ASymbol, (*const CallbackTable<(), ACallbackTableTypes>, *const ())>,
	///FIXME: This is not-at-all a fair queue.
	pub(crate) update_queue:
		BTreeMap<ASymbol, VecDeque<Box<dyn 'static + Send + FnOnce() -> Propagation>>>,
	pub(crate) stale_queue: BTreeSet<ASymbol>,
	pub(crate) interdependencies: Interdependencies,
}

impl Debug for ASignalRuntime_ {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ASignalRuntime_")
			.field("context_stack", &self.context_stack)
			.field("callbacks", &self.callbacks)
			.field("update_queue", &self.update_queue.keys())
			.field("stale_queue", &self.stale_queue)
			.field("interdependencies", &self.interdependencies)
			.finish()
	}
}

#[derive(Debug)]
pub(crate) struct Interdependencies {
	/// Note: While a symbol is flagged as subscribed explicitly,
	///       it is present as its own subscriber here (by not in `all_by_dependency`!).
	/// FIXME: This could store subscriber counts instead.
	pub(crate) subscribers_by_dependency: BTreeMap<ASymbol, BTreeSet<ASymbol>>,
	pub(crate) all_by_dependent: BTreeMap<ASymbol, BTreeSet<ASymbol>>,
	pub(crate) all_by_dependency: BTreeMap<ASymbol, BTreeSet<ASymbol>>,
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

impl ASignalRuntime {
	pub(crate) const fn new() -> Self {
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

	pub(crate) fn peek_stale<'a>(
		&self,
		borrow: RefMut<'a, ASignalRuntime_>,
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

	pub(crate) fn subscribe_to_with<'a>(
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

						borrow.context_stack.push(None);
						drop(borrow);
						let propagation =
							try_eval(|| on_subscribed_change(data, true)).finally(|()| {
								let mut borrow = (**lock).borrow_mut();
								assert_eq!(borrow.context_stack.pop(), Some(None));
							});
						borrow = (**lock).borrow_mut();
						match propagation {
							Propagation::Halt => (),
							Propagation::Propagate => {
								borrow =
									self.mark_direct_dependencies_stale(dependency, &lock, borrow);
							}
						}
					}
				}
			}
		}
		borrow
	}

	pub(crate) fn unsubscribe_from_with<'a>(
		&self,
		dependency: ASymbol,
		dependent: ASymbol,
		lock: &'a ReentrantMutexGuard<'a, RefCell<ASignalRuntime_>>,
		mut borrow: RefMut<'a, ASignalRuntime_>,
	) -> RefMut<'a, ASignalRuntime_> {
		let subscribers = &borrow
			.interdependencies
			.subscribers_by_dependency
			.entry(dependency)
			.or_default();
		if subscribers.len() == 1 && subscribers.iter().all(|s| *s == dependent) {
			// Only subscriber, so propagate upwards and then refresh first!

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
		}

		let subscribers = &mut borrow
			.interdependencies
			.subscribers_by_dependency
			.entry(dependency)
			.or_default();

		if subscribers.remove(&dependent) && subscribers.is_empty() {
			// Just removed last subscriber, so call the change handler (if there is one).

			if let Some(&(callback_table, data)) = borrow.callbacks.get(&dependency) {
				unsafe {
					if let CallbackTable {
						on_subscribed_change: Some(on_subscribed_change),
						..
					} = *callback_table
					{
						// Note: Subscribed status change handlers *may* see stale values!
						// I think simpler/deduplicated propagation is likely worth that tradeoff.

						borrow.context_stack.push(None);
						drop(borrow);
						let propagation =
							try_eval(|| on_subscribed_change(data, false)).finally(|()| {
								let mut borrow = (**lock).borrow_mut();
								assert_eq!(borrow.context_stack.pop(), Some(None));
							});
						borrow = (**lock).borrow_mut();
						match propagation {
							Propagation::Halt => (),
							Propagation::Propagate => {
								borrow =
									self.mark_direct_dependencies_stale(dependency, &lock, borrow);
							}
						}
					}
				}
			}

			// `dependency` is now unsubscribed, but should still refresh one final time
			// to ensure e.g. a reference-counted resource is properly flushed.
			borrow.context_stack.push(None);
			drop(borrow);
			//FIXME: This here is wrapped like this because `self.refresh` would otherwise process pending changes
			// (which shouldn't happen here because the dependent may just so still be subscribed). It's possible
			// to (overall) organise this better and avoid a bunch of extra work here.
			try_eval(|| self.refresh(dependency)).finally(|()| {
				let mut borrow = (**lock).borrow_mut();
				assert_eq!(borrow.context_stack.pop(), Some(None));
			});
			borrow = (**lock).borrow_mut();
		}

		borrow
	}

	pub(crate) fn process_pending<'a>(
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
						borrow = self.mark_direct_dependencies_stale(symbol, &lock, borrow)
					}
					Propagation::Halt => (),
				}
			}

			let stale;
			(stale, borrow) = self.peek_stale(borrow);
			if let Some(stale) = stale {
				try_eval(|| {
					borrow.context_stack.push(None);
					drop(borrow);
					self.refresh(stale)
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

	pub(crate) fn next_update<'a>(
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

	pub(crate) fn mark_direct_dependencies_stale<'a>(
		&self,
		id: ASymbol,
		_lock: &'a ReentrantMutexGuard<'a, RefCell<ASignalRuntime_>>,
		mut borrow: RefMut<'a, ASignalRuntime_>,
	) -> RefMut<'a, ASignalRuntime_> {
		let dependents = borrow
			.interdependencies
			.all_by_dependency
			.entry(id)
			.or_default()
			.iter()
			.copied()
			.collect::<Vec<_>>();
		borrow.stale_queue.extend(dependents);
		borrow
	}

	pub(crate) fn shrink_dependencies<'a>(
		&self,
		id: ASymbol,
		recorded_dependencies: BTreeSet<ASymbol>,
		lock: &'a ReentrantMutexGuard<'a, RefCell<ASignalRuntime_>>,
		mut borrow: RefMut<'a, ASignalRuntime_>,
	) -> RefMut<'a, ASignalRuntime_> {
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
			});

			borrow = (*lock).borrow_mut();
			borrow = match propagation {
				Propagation::Propagate => self.mark_direct_dependencies_stale(id, &lock, borrow),
				Propagation::Halt => borrow,
			};
		}

		self.process_pending(&lock, borrow);
		t
	}

	fn stop(&self, id: Self::Symbol) {
		let lock = self.critical_mutex.lock();
		let mut borrow = (*lock).borrow_mut();
		todo!()
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

	fn set_subscription(&self, id: Self::Symbol, enabled: bool) -> bool {
		let lock = self.critical_mutex.lock();
		let mut borrow = (*lock).borrow_mut();

		let is_inherently_subscribed = borrow
			.interdependencies
			.subscribers_by_dependency
			.get(&id)
			.is_some_and(|subs| subs.contains(&id));

		let result = match (enabled, is_inherently_subscribed) {
			(true, false) => {
				borrow = self.subscribe_to_with(id, id, &lock, borrow);
				true
			}
			(false, true) => {
				borrow = self.unsubscribe_from_with(id, id, &lock, borrow);
				true
			}
			(true, true) | (false, false) => false,
		};

		self.process_pending(&lock, borrow);
		result
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
					let (propagation, t) = f();
					once.set_blocking(Some(Ok(t)).into())
						.map_err(|_| ())
						.expect("unreachable");
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
				.wait()
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
			this: &ASignalRuntime,
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
				Propagation::Propagate => this.mark_direct_dependencies_stale(id, &lock, borrow),
				Propagation::Halt => borrow,
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
		dbg!(id);
		if borrow.stale_queue.remove(&id) {
			dbg!(id);
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
							borrow = self.mark_direct_dependencies_stale(id, &lock, borrow)
						}
						Propagation::Halt => (),
					}
				} else {
					// If there's no callback, then always mark dependencies as stale!
					// (This happens with uncached signals, for example.)
					borrow = self.mark_direct_dependencies_stale(id, &lock, borrow);
				}
			} else {
				// If there's no callback, then always mark dependencies as stale!
				// (This happens with uncached signals, for example.)
				borrow = self.mark_direct_dependencies_stale(id, &lock, borrow);
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

		borrow = self.unsubscribe_from_with(id, id, &lock, borrow);

		// This can unblock futures.
		drop(borrow.update_queue.remove(&id));

		let interdependencies = &mut borrow.interdependencies;
		for collection in [
			&mut interdependencies.all_by_dependency,
			&mut interdependencies.all_by_dependent,
			&mut interdependencies.subscribers_by_dependency,
		] {
			assert!(!collection
				.remove(&id)
				.is_some_and(|dependencies| !dependencies.is_empty()))
		}
		borrow.stale_queue.remove(&id);

		self.process_pending(&lock, borrow);
	}
}
