//! Low-level types for implementing [`SignalsRuntimeRef`], as well as a functional [`GlobalSignalsRuntime`].

use core::{self};
use std::{self, fmt::Debug, future::Future, mem, num::NonZeroU64};

/// Trait for handles that let signals refer to a specific runtime (instance).
///
/// [`GlobalSignalsRuntime`] provides a usable default.
///
/// # Logic
///
/// Callbacks associated with the same `id` **must not** run in parallel or nested.  
/// Callback invocations *with the same `id` **must** be totally orderable across all threads.
///
/// # Safety
///
/// Please see the 'Safety' sections on individual associated items.
pub unsafe trait SignalsRuntimeRef: Send + Sync + Clone {
	/// The signal instance key used by this [`SignalsRuntimeRef`].
	///
	/// Used to manage dependencies and callbacks.
	type Symbol: Clone + Copy + Send + Sync;

	/// Types used in callback signatures.
	type CallbackTableTypes: ?Sized + CallbackTableTypes;

	/// Creates a fresh unique [`SignalsRuntimeRef::Symbol`] for this instance.
	///
	/// Symbols are usually not interchangeable between different instances of a runtime!  
	/// Runtimes **should** detect and panic on misuse when debug-assertions are enabled.
	///
	/// # Safety
	///
	/// The return value **must** be able to uniquely identify a signal towards this runtime.  
	/// Symbols **may not** be reused even after calls to [`.stop(id)`](`SignalsRuntimeRef::stop`).
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
	/// Dependencies that are [recorded](`SignalsRuntimeRef::record_dependency`) within
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
	/// `callback_data`, but only one at a time and only before the next [`.stop(id)`](`SignalsRuntimeRef::stop`)
	/// call for the same runtime with an identical `id` completes.
	///
	/// # See also
	///
	/// [`SignalsRuntimeRef::stop`], [`SignalsRuntimeRef::purge`]
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
	/// This method **should not** remove interdependencies,
	/// just clear the callback information and pending updates for `id`.
	///
	/// The runtime **should** remove callbacks *before* cancelling pending updates.
	///
	/// # Safety
	///
	/// After this method returns, previously-scheduled callbacks for `id` **must not** run.
	///
	/// # See also
	///
	/// [`SignalsRuntimeRef::purge`]
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
	/// This function **may** panic unless called between the start of [`.start`](`SignalsRuntimeRef::start`) and [`.stop`](`SignalsRuntimeRef::stop`) for `id`.
	///
	/// # See also
	///
	/// [`SignalsRuntimeRef::purge`]
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
	/// [`SignalsRuntimeRef::purge`]
	fn set_subscription(&self, id: Self::Symbol, enabled: bool) -> bool;

	/// Submits `f` to run exclusively for `id` outside of recording dependencies.
	///
	/// The runtime **should** run `f` eventually, but **may** cancel it in response to
	/// a [`.stop(id)`](`SignalsRuntimeRef::stop`) call with the same `id``.
	///
	/// # Panics
	///
	/// This function **may** panic unless called between [`.start`](`SignalsRuntimeRef::start`) and [`.stop`](`SignalsRuntimeRef::stop`) for `id`.
	///
	/// # Safety
	///
	/// `f` **must** be dropped or consumed before the next matching [`.stop(id)`](`SignalsRuntimeRef::stop`) call returns.
	fn update_or_enqueue(&self, id: Self::Symbol, f: impl 'static + Send + FnOnce() -> Propagation);

	/// **Immediately** submits `f` to run exclusively for `id` outside of recording dependencies.
	///
	/// # Logic
	///
	/// The runtime **should** run `f` eventually, but **may** instead cancel and return it in response to
	/// a [`.stop(id)`](`SignalsRuntimeRef::stop`) or [`.purge(id)`](`SignalsRuntimeRef::purge`) call with the same `id`.  
	/// This method **must not** block indefinitely *as long as `f` doesn't*, regardless of context.  
	/// Calling [`.stop(id)`](`SignalsRuntimeRef::stop`) with matching `id` **should** cancel the update and return the [`Err`] variant.
	///
	/// # Safety
	///
	/// `f` **must not** be dropped or run after the next matching [`.stop(id)`](`SignalsRuntimeRef::stop`) call returns.  
	/// `f` **must not** be dropped or run after the [`Future`] returned by this function is dropped.
	///
	/// The hidden type returned from this method **must not** capture the elided lifetime of `&self`.  
	/// The overcapturing here appears to be a compiler limitation that can be fixed once
	/// precise capturing in RPITIT or (not quite as nicely) TAITIT lands.
	fn update_eager<'f, T: 'f + Send, F: 'f + Send + FnOnce() -> (Propagation, T)>(
		&self,
		id: Self::Symbol,
		f: F,
	) -> Self::UpdateEager<'f, T, F>;

	type UpdateEager<'f, T: 'f, F: 'f>: 'f + Send + Future<Output = Result<T, F>>;

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
	fn update_blocking<T>(&self, id: Self::Symbol, f: impl FnOnce() -> (Propagation, T)) -> T;

	/// Runs `f` exempted from any outer dependency recordings.
	fn run_detached<T>(&self, f: impl FnOnce() -> T) -> T;

	/// # Safety
	///
	/// Iff `id` is stale, its staleness **must** be cleared by running its
	/// [`update`][`CallbackTable::update`] callback before this method returns.
	fn refresh(&self, id: Self::Symbol);

	/// Removes existing callbacks, dependency relations (in either direction) associated with `id`.
	///
	/// Ones that are scheduled as a result of this are not necessarily removed!
	///
	/// # Logic
	///
	/// The runtime **should** remove callbacks *after* processing dependency changes.  
	/// The runtime **should** remove callbacks *before* cancelling pending updates.
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

#[cfg(feature = "global_signals_runtime")]
mod a_signals_runtime;

#[cfg(feature = "global_signals_runtime")]
static ISOPRENOID_GLOBAL_SIGNALS_RUNTIME: a_signals_runtime::ASignalsRuntime =
	a_signals_runtime::ASignalsRuntime::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ASymbol(pub(crate) NonZeroU64);

pub(crate) enum ACallbackTableTypes {}

impl CallbackTableTypes for ACallbackTableTypes {
	type SubscribedStatus = bool;
}

/// A plain [`SignalsRuntimeRef`] implementation that represents a static signals runtime.
///
/// 🚧 This implementation is currently not optimised. 🚧
///
/// # Features
///
/// Enable the `global_signals_runtime` Cargo feature to implement [`SignalsRuntimeRef`] for this type.
///
/// # Logic
///
/// This runtime is guaranteed to have settled whenever the last borrow of it ceases, but
/// only regarding effects originating on the current thread. Effects from other threads
/// won't necessarily be visible without external synchronisation.
///
/// (This means that in addition to transiently borrowing calls, returned [`Future`]s
/// **may** cause the [`GlobalSignalsRuntime`] not to settle until they are dropped.)
///
/// Otherwise, it makes no additional guarantees over those specified in [`SignalsRuntimeRef`]'s documentation.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlobalSignalsRuntime;

impl Debug for GlobalSignalsRuntime {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if cfg!(feature = "global_signals_runtime") {
			#[cfg(feature = "global_signals_runtime")]
			Debug::fmt(&ISOPRENOID_GLOBAL_SIGNALS_RUNTIME, f)?;
			Ok(())
		} else {
			struct Unavailable;
			impl Debug for Unavailable {
				fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
					write!(
						f,
						"(unavailable without `isoprenoid/global_signals_runtime` feature)"
					)
				}
			}

			f.debug_struct("GlobalSignalsRuntime")
				.field("state", &Unavailable)
				.finish_non_exhaustive()
		}
	}
}

/// [`SignalsRuntimeRef::Symbol`] for [`GlobalSignalsRuntime`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GSRSymbol(ASymbol);

impl Debug for GSRSymbol {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_tuple("GSRSymbol").field(&self.0 .0).finish()
	}
}

/// [`SignalsRuntimeRef::CallbackTableTypes`] for [`GlobalSignalsRuntime`].
#[repr(transparent)]
pub struct GlobalCallbackTableTypes(ACallbackTableTypes);
impl CallbackTableTypes for GlobalCallbackTableTypes {
	//SAFETY: Everything here must be the same as for `ACallbackTableTypes`!
	type SubscribedStatus = bool;
}

#[cfg(feature = "global_signals_runtime")]
unsafe impl SignalsRuntimeRef for GlobalSignalsRuntime {
	type Symbol = GSRSymbol;
	type CallbackTableTypes = GlobalCallbackTableTypes;

	fn next_id(&self) -> GSRSymbol {
		GSRSymbol((&ISOPRENOID_GLOBAL_SIGNALS_RUNTIME).next_id())
	}

	fn record_dependency(&self, id: Self::Symbol) {
		(&ISOPRENOID_GLOBAL_SIGNALS_RUNTIME).record_dependency(id.0)
	}

	unsafe fn start<T, D: ?Sized>(
		&self,
		id: Self::Symbol,
		f: impl FnOnce() -> T,
		callback_table: *const CallbackTable<D, Self::CallbackTableTypes>,
		callback_data: *const D,
	) -> T {
		(&ISOPRENOID_GLOBAL_SIGNALS_RUNTIME).start(
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
		(&ISOPRENOID_GLOBAL_SIGNALS_RUNTIME).stop(id.0)
	}

	fn update_dependency_set<T>(&self, id: Self::Symbol, f: impl FnOnce() -> T) -> T {
		(&ISOPRENOID_GLOBAL_SIGNALS_RUNTIME).update_dependency_set(id.0, f)
	}

	fn set_subscription(&self, id: Self::Symbol, enabled: bool) -> bool {
		(&ISOPRENOID_GLOBAL_SIGNALS_RUNTIME).set_subscription(id.0, enabled)
	}

	fn update_or_enqueue(
		&self,
		id: Self::Symbol,
		f: impl 'static + Send + FnOnce() -> Propagation,
	) {
		(&ISOPRENOID_GLOBAL_SIGNALS_RUNTIME).update_or_enqueue(id.0, f)
	}

	fn update_eager<'f, T: 'f + Send, F: 'f + Send + FnOnce() -> (Propagation, T)>(
		&self,
		id: Self::Symbol,
		f: F,
	) -> Self::UpdateEager<'f, T, F> {
		(&ISOPRENOID_GLOBAL_SIGNALS_RUNTIME).update_eager(id.0, f)
	}

	type UpdateEager<'f, T: 'f, F: 'f> = private::DetachedFuture<'f, Result<T, F>>;

	fn update_blocking<T>(&self, id: Self::Symbol, f: impl FnOnce() -> (Propagation, T)) -> T {
		(&ISOPRENOID_GLOBAL_SIGNALS_RUNTIME).update_blocking(id.0, f)
	}

	fn run_detached<T>(&self, f: impl FnOnce() -> T) -> T {
		(&ISOPRENOID_GLOBAL_SIGNALS_RUNTIME).run_detached(f)
	}

	fn refresh(&self, id: Self::Symbol) {
		(&ISOPRENOID_GLOBAL_SIGNALS_RUNTIME).refresh(id.0)
	}

	fn purge(&self, id: Self::Symbol) {
		(&ISOPRENOID_GLOBAL_SIGNALS_RUNTIME).purge(id.0)
	}
}

/// The `unsafe` at-runtime version of [`Callbacks`](`crate::raw::Callbacks`),
/// mainly for use between [`RawSignal`](`crate::raw::RawSignal`) and [`SignalsRuntimeRef`].
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

/// Describes types appearing in callback signatures for a particular [`SignalsRuntimeRef`] implementation.
pub trait CallbackTableTypes: 'static {
	/// A status indicating "how subscribed" a signal now is.
	///
	/// [`GlobalSignalsRuntime`] notifies only for the first and removal of the last subscription for each signal,
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

mod private {
	use std::{
		future::Future,
		pin::Pin,
		task::{Context, Poll},
	};

	use futures_lite::FutureExt;

	pub struct DetachedFuture<'f, Output: 'f>(
		pub(super) Pin<Box<dyn 'f + Send + Future<Output = Output>>>,
	);

	impl<'f, Output: 'f> Future for DetachedFuture<'f, Output> {
		type Output = Output;

		fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
			self.0.poll(cx)
		}
	}
}
