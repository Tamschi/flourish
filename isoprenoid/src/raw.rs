//! A 100% safe-Rust API to create custom signals.
//!
//! Wrap a pinned [`RawSignal`] to create signal primitives.
//!
//! > **Hint**
//! >
//! > With a projection helper like [pin-project] or [pin-project-lite],
//! > you can inline [`RawSignal`] into your wrapper without using `unsafe`.
//! >
//! > I also wrote a blog post about this topic that may be helpful: [Pinning in plain English]
//!
//! [pin-project]: https://crates.io/crates/pin-project
//! [pin-project-lite]: https://crates.io/crates/pin-project-lite
//! [Pinning in plain English]: https://blog.schichler.dev/posts/Pinning-in-plain-English/

use core::{
	fmt::{self, Debug, Formatter},
	marker::PhantomPinned,
	pin::Pin,
};
use std::{
	any::TypeId,
	collections::{btree_map::Entry, BTreeMap},
	future::Future,
	mem::{self, MaybeUninit},
	sync::{Arc, Mutex},
};

use once_slot::OnceSlot;

use crate::{
	runtime::{CallbackTable, CallbackTableTypes, Propagation, SignalsRuntimeRef},
	slot::{Slot, Token},
};

static ISOPRENOID_CALLBACK_TABLES: Mutex<
	//BTreeMap<CallbackTable<()>, Pin<Box<CallbackTable<()>>>>,
	BTreeMap<TypeId, AssertSend<*mut ()>>,
> = Mutex::new(BTreeMap::new());

struct AssertSend<T>(T);
unsafe impl<T> Send for AssertSend<T> {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct SignalId<SR: SignalsRuntimeRef> {
	id: SR::Symbol,
	runtime: SR,
}

impl<SR: SignalsRuntimeRef> SignalId<SR> {
	fn with_runtime(runtime: SR) -> Self {
		Self {
			id: runtime.next_id(),
			runtime,
		}
	}

	fn update_dependency_set<T>(&self, f: impl FnOnce() -> T) -> T {
		self.runtime.update_dependency_set(self.id, f)
	}

	unsafe fn start<T, D: ?Sized>(
		&self,
		f: impl FnOnce() -> T,
		callback: *const CallbackTable<D, SR::CallbackTableTypes>,
		callback_data: *const D,
	) -> T {
		self.runtime.start(self.id, f, callback, callback_data)
	}

	fn subscribe(&self) {
		self.runtime.subscribe(self.id)
	}

	fn unsubscribe(&self) {
		self.runtime.unsubscribe(self.id)
	}

	/// # Safety Notes
	///
	/// `self.stop(…)` also drops associated enqueued updates.
	///
	/// # Panics
	///
	/// **May** panic iff called *not* between `self.project_or_init(…)` and `self.stop_and(…)`.
	fn update_or_enqueue(&self, f: impl 'static + Send + FnOnce() -> Propagation) {
		self.runtime.update_or_enqueue(self.id, f);
	}

	fn update_eager<'f, T: 'f + Send, F: 'f + Send + FnOnce() -> (Propagation, T)>(
		&self,
		f: F,
	) -> impl 'f + Send + Future<Output = Result<T, F>> {
		self.runtime.update_eager(self.id, f)
	}

	fn update_blocking<T>(&self, f: impl FnOnce() -> (Propagation, T)) -> T {
		self.runtime.update_blocking(self.id, f)
	}

	fn refresh(&self) {
		self.runtime.refresh(self.id);
	}

	fn stop(&self) {
		self.runtime.stop(self.id)
	}

	fn purge(&self) {
		self.runtime.purge(self.id)
	}
}

mod once_slot;

/// A mid-level signal primitive that safely encapsulates most signal lifecycles.
///
/// Conceptually, this type resembles a lazy cell but with a persistent `Eager` slot.  
/// You can borrow the pin-projected `Eager` and `Lazy` values by initialising the
/// pinned [`RawSignal`] with an `init` function and static [`Callbacks`] through
/// the [`project_or_init`](`RawSignal::project_or_init`) method, with various
/// additional low-level methods that are specific to signal use.
///
/// A [`RawSignal`] can be reverted into its uninitialised state, but only by
/// purging its callbacks and subscriptions and severing its dependency relationships.
pub struct RawSignal<Eager: Sync + ?Sized, Lazy: Sync, SR: SignalsRuntimeRef> {
	handle: SignalId<SR>,
	_pinned: PhantomPinned,
	lazy: OnceSlot<Lazy>,
	eager: Eager,
}

unsafe impl<Eager: Sync + ?Sized, Lazy: Sync, SR: SignalsRuntimeRef> Sync
	for RawSignal<Eager, Lazy, SR>
{
	// Access to `eval` is synchronised through `lazy`.
}

impl<Eager: Sync + ?Sized + Debug, Lazy: Sync + Debug, SR: SignalsRuntimeRef + Debug> Debug
	for RawSignal<Eager, Lazy, SR>
where
	SR::Symbol: Debug,
{
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		f.debug_struct("RawSignal")
			.field("handle", &self.handle)
			.field("_pinned", &self._pinned)
			.field("lazy", &self.lazy)
			.field("eager", &&self.eager)
			.finish()
	}
}
impl<SR: SignalsRuntimeRef + Unpin> Unpin for RawSignal<(), (), SR> {}

impl<Eager: Sync + ?Sized, Lazy: Sync, SR: SignalsRuntimeRef> RawSignal<Eager, Lazy, SR> {
	/// Creates a new instance of [`RawSignal`].
	pub fn new(eager: Eager) -> Self
	where
		Eager: Sized,
		SR: Default,
	{
		Self::with_runtime(eager, SR::default())
	}

	/// Creates a new instance of [`RawSignal`] with the given `runtime`.
	pub fn with_runtime(eager: Eager, runtime: SR) -> Self
	where
		Eager: Sized,
	{
		Self {
			handle: SignalId::with_runtime(runtime),
			_pinned: PhantomPinned,
			lazy: OnceSlot::new(),
			eager,
		}
	}

	/// Gives plain mutable access to the contained `Eager`.
	pub fn eager_mut(&mut self) -> &mut Eager {
		&mut self.eager
	}

	/// This method borrows the pin-projected `Eager` and `Lazy` values,
	/// marking this [`RawSignal`] as dependency of the surrounding context.
	///
	/// If necessary, the `Lazy` state is first initialised by calling `init`, recording dependencies and setting up callbacks in the process.
	///
	/// This may cause the specified [`C::ON_SUBSCRIBED_CHANGE`](`Callbacks::ON_SUBSCRIBED_CHANGE`) to be called after `init`, if a subscription to this instance already exists.
	///
	/// # Safety Notes
	///
	/// `init` is called exactly once with `receiver` before this function returns for the first time for this instance.
	///
	/// After `init` returns, [`E::UPDATE`](`Callbacks::UPDATE`) and [`E::ON_SUBSCRIBED_CHANGE`](`Callbacks::ON_SUBSCRIBED_CHANGE`)
	/// may be called any number of times with the state initialised by `init`, but at most once at a time.
	///
	/// [`RawSignal`]'s [`Drop`] implementation first prevents further callback calls and waits for running ones to finish, then drops both values in place (lazy first).
	pub fn project_or_init<C: Callbacks<Eager, Lazy, SR>>(
		self: Pin<&Self>,
		init: impl for<'b> FnOnce(Pin<&'b Eager>, Slot<'b, Lazy>) -> Token<'b>,
	) -> (Pin<&Eager>, Pin<&Lazy>) {
		self.handle.runtime.record_dependency(self.handle.id);
		unsafe {
			let eager = Pin::new_unchecked(&self.eager);
			let lazy = self.lazy.get_or_write(|cell| {
				self.handle.start(
					|| {
						let mut lazy = MaybeUninit::uninit();
						init(eager, Slot::new(&mut lazy));
						cell.set(lazy.assume_init())
							.map_err(|_| ())
							.expect("Assured by `OnceSlot` synchronisation.");
					},
					{
						let guard = &mut ISOPRENOID_CALLBACK_TABLES.lock().expect("unreachable");
						match match match guard.entry(TypeId::of::<SR::CallbackTableTypes>()) {
							Entry::Vacant(vacant) => vacant.insert(AssertSend(
								(Box::leak(Box::new(BTreeMap::<
									CallbackTable<(), SR::CallbackTableTypes>,
									Pin<Box<CallbackTable<(), SR::CallbackTableTypes>>>,
								>::new()))
									as *mut BTreeMap<
										CallbackTable<(), SR::CallbackTableTypes>,
										Pin<Box<CallbackTable<(), SR::CallbackTableTypes>>>,
									>)
									.cast::<()>(),
							)),
							Entry::Occupied(cached) => cached.into_mut(),
						} {
							AssertSend(ptr) => &mut *ptr.cast::<BTreeMap<
								CallbackTable<(), SR::CallbackTableTypes>,
								Pin<Box<CallbackTable<(), SR::CallbackTableTypes>>>,
							>>(),
						}
						.entry(
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
						}
					},
					(Pin::into_inner_unchecked(self) as *const Self).cast(),
				);

				unsafe fn update<
					Eager: Sync + ?Sized,
					Lazy: Sync,
					SR: SignalsRuntimeRef,
					C: Callbacks<Eager, Lazy, SR>,
				>(
					this: *const RawSignal<Eager, Lazy, SR>,
				) -> Propagation {
					let this = &*this;
					C::UPDATE.expect("unreachable")(
						Pin::new_unchecked(&this.eager),
						Pin::new_unchecked(this.lazy.get().expect("unreachable")),
					)
				}

				unsafe fn on_subscribed_change<
					Eager: Sync + ?Sized,
					Lazy: Sync,
					SR: SignalsRuntimeRef,
					C: Callbacks<Eager, Lazy, SR>,
				>(
					this: *const RawSignal<Eager, Lazy, SR>,
					subscribed: <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
				) -> Propagation {
					let this = &*this;
					C::ON_SUBSCRIBED_CHANGE.expect("unreachable")(
						Pin::new_unchecked(this),
						Pin::new_unchecked(&this.eager),
						Pin::new_unchecked(this.lazy.get().expect("unreachable")),
						subscribed,
					)
				}
			});
			self.handle.refresh();
			mem::transmute((eager, Pin::new_unchecked(lazy)))
		}
	}

	/// Increases this [`RawSignal`]'s intrinsic subscription count.
	pub fn subscribe(&self) {
		self.handle.subscribe()
	}

	/// Decreases this [`RawSignal`]'s intrinsic subscription count.
	///
	/// # Logic
	///
	/// Attempting to decrease the net number of intrinsic subscriptions below zero
	/// **may** cause unexpected behaviour (but not undefined behaviour).
	///
	/// # Panics
	///
	/// Attempting to decrease the net number of intrinsic subscriptions below zero
	/// **may** panic.
	pub fn unsubscribe(&self) {
		self.handle.unsubscribe()
	}

	/// Schedules access to the pinned `Eager` and `Lazy` without waiting for completion.
	///
	/// # Safety Notes
	///
	/// [`stop`](`RawSignal::stop`) also drops associated enqueued updates.
	///
	/// # Panics
	///
	/// **May** panic iff called *not* between [`project_or_init`](`RawSignal::project_or_init`) and [`stop`](`RawSignal::stop`).
	pub fn update(
		self: Pin<&Self>,
		f: impl 'static + Send + FnOnce(Pin<&Eager>, Option<Pin<&Lazy>>) -> Propagation,
	) {
		let this = Pin::clone(&self);
		let update: Box<dyn Send + FnOnce() -> Propagation> = Box::new(move || unsafe {
			f(
				this.map_unchecked(|this| &this.eager),
				this.lazy.get().map(|lazy| Pin::new_unchecked(lazy)),
			)
		});
		let update: Box<dyn 'static + Send + FnOnce() -> Propagation> =
			unsafe { mem::transmute(update) };
		self.handle.update_or_enqueue(update);
	}

	/// Immediately schedules access to `Eager` and `Lazy`.
	///
	/// Instead of pinning, `self` is borrowed for the lifetime of the future.
	///
	/// # Returns
	///
	/// A [`Future`] handle that becomes [`Poll::Ready`](`core::task::Poll::Ready`) when the scheduled update is complete or cancelled.
	///
	/// Drop this handle to cancel the update if still possible.
	///
	/// # Safety Notes
	///
	/// [`stop`](`RawSignal::stop`) also drops associated enqueued updates.
	///
	/// # Panics
	///
	/// **May** panic iff called *not* between [`project_or_init`](`RawSignal::project_or_init`) and [`stop`](`RawSignal::stop`).
	pub fn update_eager<
		'f,
		T: 'f + Send,
		F: 'f + Send + FnOnce(&Eager, Option<&Lazy>) -> (Propagation, T),
	>(
		&'f self,
		f: F,
	) -> impl 'f + Send + Future<Output = Result<T, F>>
	where
		Eager: 'f,
		Lazy: 'f,
	{
		let eager = &self.eager;
		let lazy = AssertSend(&self.lazy as *const OnceSlot<Lazy>);
		let f = Arc::new(Mutex::new(Some(f)));

		struct AssertSend<T: ?Sized>(*const T);
		unsafe impl<T: ?Sized> Send for AssertSend<T> {}
		impl<T: ?Sized> AssertSend<T> {
			unsafe fn get(&self) -> &T {
				&*self.0
			}
		}

		let future = self.handle.update_eager({
			let f = Arc::clone(&f);
			move || {
				let f = f
					.try_lock()
					.expect("unreachable")
					.take()
					.expect("unreachable");
				f(eager, unsafe { lazy.get().get() })
			}
		});
		async move {
			future.await.map_err(move |_| {
				Arc::try_unwrap(f)
					.map_err(|_| ())
					.expect("must be exclusive now")
					.into_inner()
					.expect("can't be poisoned")
					.expect("must be Some")
			})
		}
	}

	/// Immediately schedules access to the pinned `Eager` and `Lazy`.
	///
	/// # Returns
	///
	/// A [`Future`] handle that becomes [`Poll::Ready`](`core::task::Poll::Ready`) when the scheduled update is complete or cancelled.
	///
	/// Drop this handle to cancel the update if still possible.
	///
	/// # Safety Notes
	///
	/// [`stop`](`RawSignal::stop`) also drops associated enqueued updates.
	///
	/// # Panics
	///
	/// **May** panic iff called *not* between [`project_or_init`](`RawSignal::project_or_init`) and [`stop`](`RawSignal::stop`).
	pub fn update_eager_pin<
		'f,
		T: 'f + Send,
		F: 'f + Send + FnOnce(Pin<&Eager>, Option<Pin<&Lazy>>) -> (Propagation, T),
	>(
		self: Pin<&Self>,
		f: F,
	) -> impl 'f + Send + Future<Output = Result<T, F>>
	where
		Eager: 'f,
		Lazy: 'f,
	{
		let eager = AssertSend(&self.eager as *const Eager);
		let lazy = AssertSend(&self.lazy as *const OnceSlot<Lazy>);
		let f = Arc::new(Mutex::new(Some(f)));

		struct AssertSend<T: ?Sized>(*const T);
		unsafe impl<T: ?Sized> Send for AssertSend<T> {}
		impl<T: ?Sized> AssertSend<T> {
			unsafe fn get(&self) -> &T {
				&*self.0
			}
		}

		let future = self.handle.update_eager({
			let f = Arc::clone(&f);
			move || {
				let f = f
					.try_lock()
					.expect("unreachable")
					.take()
					.expect("unreachable");
				unsafe {
					f(
						Pin::new_unchecked(eager.get()),
						lazy.get().get().map(|r| Pin::new_unchecked(r)),
					)
				}
			}
		});
		async move {
			future.await.map_err(move |_| {
				Arc::try_unwrap(f)
					.map_err(|_| ())
					.expect("must be exclusive now")
					.into_inner()
					.expect("can't be poisoned")
					.expect("must be Some")
			})
		}
	}

	/// Synchronously gives access to the `Eager` and `Lazy` *without pinning*.
	///
	/// # Deadlocks
	///
	/// This function **may** easily deadlock iff called in a signal-related callback.
	///
	/// > If in doubt, don't!
	///
	/// # Panics
	///
	/// **May** panic iff called *not* between [`project_or_init`](`RawSignal::project_or_init`) and [`stop`](`RawSignal::stop`).
	///
	/// **May** panic iff called in a signal-related callback.
	pub fn update_blocking<T>(
		&self,
		f: impl FnOnce(&Eager, Option<&Lazy>) -> (Propagation, T),
	) -> T {
		self.handle
			.update_blocking(move || f(&self.eager, self.lazy.get()))
	}

	/// Synchronously gives access to the `Eager` and `Lazy`.
	///
	/// # Deadlocks
	///
	/// This function **may** easily deadlock iff called in a signal-related callback.
	///
	/// > If in doubt, don't!
	///
	/// # Panics
	///
	/// **May** panic iff called *not* between [`project_or_init`](`RawSignal::project_or_init`) and [`stop`](`RawSignal::stop`).
	///
	/// **May** panic iff called in a signal-related callback.
	pub fn update_blocking_pin<T>(
		self: Pin<&Self>,
		f: impl FnOnce(Pin<&Eager>, Option<Pin<&Lazy>>) -> (Propagation, T),
	) -> T {
		self.handle.update_blocking(move || unsafe {
			f(
				self.map_unchecked(|this| &this.eager),
				self.lazy.get().map(|lazy| Pin::new_unchecked(lazy)),
			)
		})
	}

	/// Safe wrapper for [`SignalsRuntimeRef::update_dependency_set`]
	/// that gives access to the `Eager` and `Lazy`.
	pub fn update_dependency_set<T>(
		self: Pin<&Self>,
		f: impl FnOnce(Pin<&Eager>, Pin<&Lazy>) -> T,
	) -> T {
		self.handle.update_dependency_set(move || unsafe {
			f(
				Pin::new_unchecked(&self.eager),
				Pin::new_unchecked(match self.lazy.get() {
					Some(lazy) => lazy,
					None => panic!(
						"`RawSignal::update_dependency_set` may only be used after initialisation."
					),
				}),
			)
		})
	}

	/// Wraps [`SR::clone`](`Clone::clone`).
	pub fn clone_runtime_ref(&self) -> SR {
		self.handle.runtime.clone()
	}

	/// Wraps [`SignalsRuntimeRef::stop`].
	pub fn stop(&self) {
		self.handle.stop();
	}

	/// Instructs the signals runtime to release all resources associated with this [`RawSignal`],
	/// then, if initialised, drops the `Lazy` after calling `before_deinit`.
	///
	/// Note that calling this function *isn't* always necessary to avoid leaks,
	/// as [`RawSignal::drop`] also calls through to [`SignalsRuntimeRef::purge`].
	pub fn purge_and_deinit_with<T>(
		self: Pin<&mut Self>,
		before_deinit: impl FnOnce(Pin<&Eager>, Pin<&mut Lazy>) -> T,
	) -> Option<T> {
		self.handle.purge();
		if self.lazy.get().is_some() {
			unsafe {
				//SAFETY: Once `handle` has been purged, `self` isn't aliased anymore,
				//        so it's now safe to get mutable access.
				let this = Pin::into_inner_unchecked(self);
				let t = before_deinit(
					Pin::new_unchecked(&this.eager),
					Pin::new_unchecked(this.lazy.get_mut().expect("unreachable")),
				);
				// `lazy` is pinned, so overwrite it in place.
				this.lazy = OnceSlot::new();
				Some(t)
			}
		} else {
			None
		}
	}
}

/// 1. Instructs the runtime to release all resources associated with this [`RawSignal`].
/// 2. Drops the `Lazy`, iff initialised.
/// 3. Drops the `Eager`.
impl<Eager: Sync + ?Sized, Lazy: Sync, SR: SignalsRuntimeRef> Drop for RawSignal<Eager, Lazy, SR> {
	fn drop(&mut self) {
		if self.lazy.get().is_some() {
			self.handle.purge()
		}
	}
}

/// Describes static callback tables used to set up each [`RawSignal`].
///
/// For each [`RawSignal`] instance, these functions are called altogether at most once at a time.
pub trait Callbacks<Eager: ?Sized + Sync, Lazy: Sync, SR: SignalsRuntimeRef> {
	/// The primary update callback for signals. Whenever a signal has internally cached state,
	/// it should specify an [`UPDATE`](`Callbacks::UPDATE`) handler to recompute it.
	///
	/// # Logic Notes
	///
	/// If this is [`None`] or the [`RawSignal`] isn't subscribed to,
	/// the runtime (implicitly) **should** always propagate staleness
	/// to dependents *without* refreshing the [`RawSignal`].
	///
	/// The runtime (implicitly) **must** record dependencies for this callback and update them for this [`RawSignal`] afterwards.
	///
	/// # Safety
	///
	/// Only called once at a time for each initialised [`RawSignal`], and not concurrently with [`ON_SUBSCRIBED_CHANGE`](`Callbacks::ON_SUBSCRIBED_CHANGE`).
	const UPDATE: Option<fn(eager: Pin<&Eager>, lazy: Pin<&Lazy>) -> Propagation>;

	/// A subscription change notification callback.
	///
	/// # Logic
	///
	/// The runtime **must** consider transitive subscriptions.  
	/// The runtime **must** consider a signal's own intrinsic subscriptions.  
	/// The runtime **must not** run this function while recording dependencies (but may start a nested recording in response to the callback).
	///
	/// # Safety
	///
	/// Only called once at a time for each initialised [`RawSignal`], and not concurrently with [`UPDATE`](`Callbacks::UPDATE`).
	const ON_SUBSCRIBED_CHANGE: Option<
		fn(
			source: Pin<&RawSignal<Eager, Lazy, SR>>,
			eager: Pin<&Eager>,
			lazy: Pin<&Lazy>,
			subscribed: <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
		) -> Propagation,
	>;
}

/// A vacant [`Callbacks`] implementation that specifies [`None`] for all callbacks.  
/// (Callbacks are called dynamically by the [`SignalsRuntimeRef`], so [`None`] helps to skip locks in some circumstances.)
///
/// When using this [`Callbacks`] implementation, updates (implicitly) **should** still propagate to dependent signals.
pub enum NoCallbacks {}
impl<Eager: ?Sized + Sync, Lazy: Sync, SR: SignalsRuntimeRef> Callbacks<Eager, Lazy, SR>
	for NoCallbacks
{
	const UPDATE: Option<fn(eager: Pin<&Eager>, lazy: Pin<&Lazy>) -> Propagation> = None;
	const ON_SUBSCRIBED_CHANGE: Option<
		fn(
			source: Pin<&RawSignal<Eager, Lazy, SR>>,
			eager: Pin<&Eager>,
			lazy: Pin<&Lazy>,
			subscribed: <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
		) -> Propagation,
	> = None;
}
