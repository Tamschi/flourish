//! Wrap a [`RawSignal`] to create signal primitives.

use core::{
	fmt::{self, Debug, Formatter},
	marker::PhantomPinned,
	pin::Pin,
};
use std::{
	any::TypeId,
	cell::UnsafeCell,
	collections::{btree_map::Entry, BTreeMap},
	mem::{self, MaybeUninit},
	sync::{Mutex, OnceLock},
};

use crate::{
	runtime::{CallbackTable, CallbackTableTypes, Propagation, SignalRuntimeRef},
	slot::{Slot, Token},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct SignalId<SR: SignalRuntimeRef> {
	id: SR::Symbol,
	runtime: SR,
}

impl<SR: SignalRuntimeRef> SignalId<SR> {
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

	fn set_subscription(&self, enabled: bool) -> bool {
		self.runtime.set_subscription(self.id, enabled)
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

	async fn update_async<T: Send, F: Send + FnOnce() -> (Propagation, T)>(
		&self,
		f: F,
	) -> Result<T, F> {
		self.runtime.update_async(self.id, f).await
	}

	fn update_blocking<T>(&self, f: impl FnOnce() -> (Propagation, T)) -> T {
		self.runtime.update_blocking(self.id, f)
	}

	fn refresh(&self) {
		self.runtime.refresh(self.id);
	}

	fn purge(&self) {
		self.runtime.purge(self.id)
	}
}

/// A slightly higher-level signal primitive than using a runtime's [`SignalRuntimeRef::Symbol`] directly.
/// This type comes with some lifecycle management to ensure orderly callbacks and safe data access.
pub struct RawSignal<Eager: Sync + ?Sized, Lazy: Sync, SR: SignalRuntimeRef> {
	handle: SignalId<SR>,
	_pinned: PhantomPinned,
	lazy: UnsafeCell<OnceLock<Lazy>>,
	eager: Eager,
}

unsafe impl<Eager: Sync + ?Sized, Lazy: Sync, SR: SignalRuntimeRef> Sync
	for RawSignal<Eager, Lazy, SR>
{
	// Access to `eval` is synchronised through `lazy`.
}

impl<Eager: Sync + ?Sized + Debug, Lazy: Sync + Debug, SR: SignalRuntimeRef + Debug> Debug
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
impl<SR: SignalRuntimeRef + Unpin> Unpin for RawSignal<(), (), SR> {}

impl<Eager: Sync + ?Sized, Lazy: Sync, SR: SignalRuntimeRef> RawSignal<Eager, Lazy, SR> {
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
			handle: SignalId::with_runtime(runtime),
			_pinned: PhantomPinned,
			lazy: OnceLock::new().into(),
			eager: eager.into(),
		}
	}

	pub fn eager_mut(&mut self) -> &mut Eager {
		&mut self.eager
	}

	/// Initialises this [`RawSignal`]'s lazy state is initialised if necessary, recording dependencies in the process.
	///
	/// This may cause [`C::ON_SUBSCRIBED_CHANGE`](`Callbacks::ON_SUBSCRIBED_CHANGE`) to be called after `init` if a subscription to this instance already exists.
	///
	/// This [`RawSignal`] is marked as dependency of the surrounding context, iff any, which may also cause callbacks to be called.
	///
	/// # Safety Notes
	///
	/// `init` is called exactly once with `receiver` before this function returns for the first time for this instance.
	///
	/// After `init` returns, [`E::UPDATE`](`Callbacks::UPDATE`) and [`E::ON_SUBSCRIBED_CHANGE`](`Callbacks::ON_SUBSCRIBED_CHANGE`)
	/// may be called any number of times with the state initialised by `init`, but at most once at a time.
	///
	/// [`RawSignal`]'s [`Drop`] implementation first prevents further `eval` calls and waits for running ones to finish (not necessarily in this order), then drops the `T` in place.
	pub fn project_or_init<C: Callbacks<Eager, Lazy, SR>>(
		self: Pin<&Self>,
		init: impl for<'b> FnOnce(Pin<&'b Eager>, Slot<'b, Lazy>) -> Token<'b>,
	) -> (Pin<&Eager>, Pin<&Lazy>) {
		self.handle.runtime.record_dependency(self.handle.id);
		unsafe {
			let eager = Pin::new_unchecked(&self.eager);
			let lazy = (&*self.lazy.get()).get_or_init(|| {
				let mut lazy = MaybeUninit::uninit();
				self.handle.start(
					|| drop(init(eager, Slot::new(&mut lazy))),
					{
						let guard = &mut CALLBACK_TABLES.lock().expect("unreachable");
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

				struct AssertSend<T>(T);
				unsafe impl<T> Send for AssertSend<T> {}

				static CALLBACK_TABLES: Mutex<
					//BTreeMap<CallbackTable<()>, Pin<Box<CallbackTable<()>>>>,
					BTreeMap<TypeId, AssertSend<*mut ()>>,
				> = Mutex::new(BTreeMap::new());

				unsafe fn update<
					Eager: Sync + ?Sized,
					Lazy: Sync,
					SR: SignalRuntimeRef,
					C: Callbacks<Eager, Lazy, SR>,
				>(
					this: *const RawSignal<Eager, Lazy, SR>,
				) -> Propagation {
					let this = &*this;
					C::UPDATE.expect("unreachable")(
						Pin::new_unchecked(&this.eager),
						Pin::new_unchecked((&*this.lazy.get()).get().expect("unreachable")),
					)
				}

				unsafe fn on_subscribed_change<
					Eager: Sync + ?Sized,
					Lazy: Sync,
					SR: SignalRuntimeRef,
					C: Callbacks<Eager, Lazy, SR>,
				>(
					this: *const RawSignal<Eager, Lazy, SR>,
					subscribed: <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
				) -> Propagation {
					let this = &*this;
					C::ON_SUBSCRIBED_CHANGE.expect("unreachable")(
						Pin::new_unchecked(this),
						Pin::new_unchecked(&this.eager),
						Pin::new_unchecked((&*this.lazy.get()).get().expect("unreachable")),
						subscribed,
					)
				}

				lazy.assume_init()
			});
			self.handle.refresh();
			mem::transmute((eager, Pin::new_unchecked(lazy)))
		}
	}

	/// Tries to newly inherently subscribe this [`RawSignal`].
	///
	/// Iff an inherent subscription already exists, this method returns [`None`] and has no logic side-effects.
	///
	/// Otherwise (iff the inherent subscription is new), this [`RawSignal`]'s lazy state is initialised if necessary,
	/// recording dependencies in the process. This also causes [`C::ON_SUBSCRIBED_CHANGE`](`Callbacks::ON_SUBSCRIBED_CHANGE`)
	/// to be called after `init` due to the already-present subscription, but without recording dependencies.
	///
	/// (Iff the subscription is new but initialisation is not necessary, then the previously-configured [`Callbacks::ON_SUBSCRIBED_CHANGE`] is used.)
	///
	/// This [`RawSignal`] is marked as dependency of the surrounding context, iff any.
	///
	/// # Safety Notes
	///
	/// `init` is called exactly once with `receiver` before this function returns for the first time for this instance.
	///
	/// After `init` returns, [`E::UPDATE`](`Callbacks::UPDATE`) and [`E::ON_SUBSCRIBED_CHANGE`](`Callbacks::ON_SUBSCRIBED_CHANGE`)
	/// may be called any number of times with the state initialised by `init`, but at most once at a time.
	///
	/// [`RawSignal`]'s [`Drop`] implementation first prevents further `eval` calls and waits for running ones to finish (not necessarily in this order), then drops the `T` in place.
	pub fn subscribe_inherently_or_init<C: Callbacks<Eager, Lazy, SR>>(
		self: Pin<&Self>,
		init: impl for<'b> FnOnce(Pin<&'b Eager>, Slot<'b, Lazy>) -> Token<'b>,
	) -> Option<(Pin<&Eager>, Pin<&Lazy>)> {
		self.handle.runtime.record_dependency(self.handle.id);
		unsafe {
			let eager = Pin::new_unchecked(&self.eager);
			let lazy = self.handle.set_subscription(true).then(|| {
				let lazy = (&*self.lazy.get()).get_or_init(|| {
					let mut lazy = MaybeUninit::uninit();
					self.handle.start(
						|| drop(init(eager, Slot::new(&mut lazy))),
						{
							let guard = &mut CALLBACK_TABLES.lock().expect("unreachable");
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
									update: C::UPDATE
										.is_some()
										.then_some(update::<Eager, Lazy, SR, C>),
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

					struct AssertSend<T>(T);
					unsafe impl<T> Send for AssertSend<T> {}

					static CALLBACK_TABLES: Mutex<
						//BTreeMap<CallbackTable<()>, Pin<Box<CallbackTable<()>>>>,
						BTreeMap<TypeId, AssertSend<*mut ()>>,
					> = Mutex::new(BTreeMap::new());

					unsafe fn update<
						Eager: Sync + ?Sized,
						Lazy: Sync,
						SR: SignalRuntimeRef,
						C: Callbacks<Eager, Lazy, SR>,
					>(
						this: *const RawSignal<Eager, Lazy, SR>,
					) -> Propagation {
						let this = &*this;
						C::UPDATE.expect("unreachable")(
							Pin::new_unchecked(&this.eager),
							Pin::new_unchecked((&*this.lazy.get()).get().expect("unreachable")),
						)
					}

					unsafe fn on_subscribed_change<
						Eager: Sync + ?Sized,
						Lazy: Sync,
						SR: SignalRuntimeRef,
						C: Callbacks<Eager, Lazy, SR>,
					>(
						this: *const RawSignal<Eager, Lazy, SR>,
						subscribed: <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
					) -> Propagation {
						let this = &*this;
						C::ON_SUBSCRIBED_CHANGE.expect("unreachable")(
							Pin::new_unchecked(this),
							Pin::new_unchecked(&this.eager),
							Pin::new_unchecked((&*this.lazy.get()).get().expect("unreachable")),
							subscribed,
						)
					}

					lazy.assume_init()
				});
				lazy
			});
			self.handle.refresh();
			lazy.map(|lazy| mem::transmute((eager, Pin::new_unchecked(lazy))))
		}
	}

	/// Unsubscribes this [`RawSignal`] (only regarding innate subscription!).
	///
	/// # Returns
	///
	/// Whether this instance was previously innately subscribed.
	///
	/// An innate subscription is a subscription not caused by a dependent subscriber.
	pub fn unsubscribe_inherently(self: Pin<&Self>) -> bool {
		self.handle.set_subscription(false)
	}

	/// # Safety Notes
	///
	/// `self.stop(…)` also drops associated enqueued updates.
	///
	/// # Panics
	///
	/// **May** panic iff called *not* between `self.start(…)` and `self.stop(…)`.
	pub fn update(
		self: Pin<&Self>,
		f: impl 'static + Send + FnOnce(Pin<&Eager>, Option<Pin<&Lazy>>) -> Propagation,
	) where
		SR::Symbol: Sync,
	{
		let this = Pin::clone(&self);
		let update: Box<dyn Send + FnOnce() -> Propagation> = Box::new(move || unsafe {
			f(
				this.map_unchecked(|this| &this.eager),
				(&*this.lazy.get())
					.get()
					.map(|lazy| Pin::new_unchecked(lazy)),
			)
		});
		let update: Box<dyn 'static + Send + FnOnce() -> Propagation> =
			unsafe { mem::transmute(update) };
		self.handle.update_or_enqueue(update);
	}

	pub async fn update_async<T: Send>(
		&self,
		f: impl Send + FnOnce(&Eager, Option<&Lazy>) -> (Propagation, T),
	) -> T {
		let update: Box<dyn Send + FnOnce() -> (Propagation, T)> =
			Box::new(move || f(&self.eager, unsafe { &*self.lazy.get() }.get()));
		let update: Box<dyn 'static + Send + FnOnce() -> (Propagation, T)> =
			unsafe { mem::transmute(update) };
		self.handle
            .update_async(update)
            .await
            .map_err(|_| ())
            .expect("Cancelling the update in a way this here would be reached would require calling `.stop()`, which isn't possible here because that requires an exclusive/`mut` reference.")
	}

	pub async fn update_async_pin<T: Send>(
		self: Pin<&Self>,
		f: impl Send + FnOnce(Pin<&Eager>, Option<Pin<&Lazy>>) -> (Propagation, T),
	) -> T {
		let this = Pin::clone(&self);
		let update: Box<dyn Send + FnOnce() -> (Propagation, T)> = Box::new(move || unsafe {
			f(
				this.map_unchecked(|this| &this.eager),
				(&*this.lazy.get())
					.get()
					.map(|lazy| Pin::new_unchecked(lazy)),
			)
		});
		let update: Box<dyn 'static + Send + FnOnce() -> (Propagation, T)> =
			unsafe { mem::transmute(update) };
		self.handle
            .update_async(update)
            .await
            .map_err(|_| ())
            .expect("Cancelling the update in a way this here would be reached would require calling `.stop()`, which isn't possible here because that requires an exclusive/`mut` reference.")
	}

	pub fn update_blocking<T>(
		&self,
		f: impl FnOnce(&Eager, Option<&Lazy>) -> (Propagation, T),
	) -> T {
		self.handle
			.update_blocking(move || f(&self.eager, unsafe { &*self.lazy.get() }.get()))
	}

	pub fn update_blocking_pin<T>(
		self: Pin<&Self>,
		f: impl FnOnce(Pin<&Eager>, Option<Pin<&Lazy>>) -> (Propagation, T),
	) -> T {
		self.handle.update_blocking(move || unsafe {
			f(
				self.map_unchecked(|this| &this.eager),
				(&*self.lazy.get())
					.get()
					.map(|lazy| Pin::new_unchecked(lazy)),
			)
		})
	}

	pub fn update_dependency_set<T, F: FnOnce(Pin<&Eager>, Pin<&Lazy>) -> T>(
		self: Pin<&Self>,
		f: F,
	) -> T {
		self.handle.update_dependency_set(move || unsafe {
			f(
				Pin::new_unchecked(&self.eager),
				Pin::new_unchecked(match (&*self.lazy.get()).get() {
					Some(lazy) => lazy,
					None => panic!("`RawSignal::track` may only be used after initialisation."),
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

	pub fn deinit_and<T>(
		self: Pin<&mut Self>,
		f: impl FnOnce(Pin<&Eager>, Pin<&mut Lazy>) -> T,
	) -> Option<T> {
		if unsafe { &*self.lazy.get() }.get().is_some() {
			self.handle.purge();
			let t = f(unsafe { Pin::new_unchecked(&self.eager) }, unsafe {
				Pin::new_unchecked((&mut *self.lazy.get()).get_mut().expect("unreachable"))
			});
			unsafe { *self.lazy.get() = OnceLock::new() };
			Some(t)
		} else {
			None
		}
	}
}

impl<Eager: Sync + ?Sized, Lazy: Sync, SR: SignalRuntimeRef> Drop for RawSignal<Eager, Lazy, SR> {
	fn drop(&mut self) {
		if unsafe { &*self.lazy.get() }.get().is_some() {
			self.handle.purge()
		}
	}
}

/// Static callback tables used to set up each [`RawSignal`].
///
/// For each [`RawSignal`] instance, these functions are called altogether at most once at a time.
pub trait Callbacks<Eager: ?Sized + Sync, Lazy: Sync, SR: SignalRuntimeRef> {
	/// The primary update callback for signals. Whenever a signal has internally cached state,
	/// it should specify an [`UPDATE`](`Callbacks::UPDATE`) handler to recompute it.
	///
	/// **Note:** At least with the default runtime, the stale flag *always* propagates while this is [`None`] or there are no active subscribers.
	///
	/// # Safety
	///
	/// Only called once at a time for each initialised [`RawSignal`].
	const UPDATE: Option<fn(eager: Pin<&Eager>, lazy: Pin<&Lazy>) -> Propagation>;

	/// A subscription change notification callback.
	///
	/// # Logic
	///
	/// The runtime **must** consider transitive subscriptions.  
	/// The runtime **must** consider a signal's own inherent subscription.  
	/// The runtime **must not** run this function while recording dependencies (but may start a nested recording in response to the callback).
	///
	/// # Safety
	///
	/// Only called once at a time for each initialised [`RawSignal`], and not concurrently with [`Self::UPDATE`].
	const ON_SUBSCRIBED_CHANGE: Option<
		fn(
			source: Pin<&RawSignal<Eager, Lazy, SR>>,
			eager: Pin<&Eager>,
			lazy: Pin<&Lazy>,
			subscribed: <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
		) -> Propagation,
	>;
}

/// A [`Callbacks`] implementation that only specifies [`None`].
///
/// When using this [`Callbacks`], updates still propagate to dependent signals.
///
/// Callbacks are internally type-erased, so [`None`] helps to skip locks in some circumstances.
pub enum NoCallbacks {}
impl<Eager: ?Sized + Sync, Lazy: Sync, SR: SignalRuntimeRef> Callbacks<Eager, Lazy, SR>
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
