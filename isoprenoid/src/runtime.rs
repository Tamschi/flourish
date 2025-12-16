//! Low-level types for implementing [`SignalsRuntimeRef`], as well as [`GlobalSignalsRuntime`].
//!
//! # Features
//!
//! Enable the `global_signals_runtime` Cargo feature for [`GlobalSignalsRuntime`] to implement [`SignalsRuntimeRef`].

use core::{self};
use std::{
	self,
	fmt::{self, Debug, Formatter},
	future::Future,
	mem,
	num::NonZeroU64,
};

/// Embedded in signals to refer to a specific signals runtime.
///
/// The signals runtime determines when its associated signals are refreshed in response to dependency changes.
///
/// [`GlobalSignalsRuntime`] provides a usable default.
///
/// # Logic
/// Callback invocations associated with the same `id` **must** be totally orderable across all threads.
///
/// # Safety
///
/// Callbacks associated with the same `id` **must not** run concurrently but **may** be nested in some cases.  
///
/// Please see the 'Safety' sections on this trait's associated items for additional rules.
///
/// Iff equivalent [`SignalsRuntimeRef`] instances may be accessed concurrently,
/// the runtime **must** handle concurrent method calls with the same `id` gracefully.
///
/// The runtime **must** behave as if method calls associate with the same `id` were totally orderable.  
/// The runtime **may** decide the effective order of concurrent calls arbitrarily.
///
/// ## Definition
///
/// An `id` is considered 'started' exactly between the start of each associated call to
/// [`start`](`SignalsRuntimeRef::start`) and a runtime-specific point during the following
/// associated [`stop`](`SignalsRuntimeRef::stop`) call after which no associated callbacks
/// may still be executed by the runtime.
///
/// '`id`'s context' is the execution context of any methods on this trait where the same `id`
/// is used as parameter and additionally that of callbacks associated with `id`.
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
	///
	/// Symbols **may** be reused by signals even after [`stop`](`SignalsRuntimeRef::stop`) and
	/// as such **must not** be reallocated by a given runtime.
	fn next_id(&self) -> Self::Symbol;

	/// When run in a context that records dependencies, records `id` as dependency of that context.
	///
	/// # Logic
	///
	/// If a call to [`record_dependency`](`SignalsRuntimeRef::record_dependency`) causes a subscription
	/// change, the runtime **should** call that [`CallbackTable::on_subscribed_change`] callback before
	/// returning from this function. (This helps to manage on-demand-only resources more efficiently.)
	///
	/// This method **must** function even for an otherwise unknown `id` as long as it was allocated by [`next_id`](`SignalsRuntimeRef::next_id`).
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
	/// Before this method returns, `init` **must** have been called synchronously.
	///
	/// Only after `init` completes, the runtime **may** run the functions specified in `callback_table` with
	/// `callback_data` any number of times and in any order, but only one at a time and only before the next
	/// [`.stop(id)`](`SignalsRuntimeRef::stop`) call on `self` with an identical `id` completes.
	///
	/// # Panics
	///
	/// This method **may** panic if called when `id` is already started.
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
	/// Calls to [`stop`](`SignalsRuntimeRef::stop`) made while `id` is not started
	/// **must** return normally and **should not** have observable effects outside diagnostics.
	///
	/// # Safety
	///
	/// After this method returns normally, previously-scheduled callbacks for `id` **must not** run.
	///
	/// Iff this method instead panics, then `id` **must** still be considered started
	/// and `callback_data` **may** still be accessed.
	///
	/// # Panics
	///
	/// This method **should** panic if called in `id`'s context.  
	/// (The call **may** instead deadlock.)
	///
	/// # See also
	///
	/// [`SignalsRuntimeRef::purge`]
	fn stop(&self, id: Self::Symbol);

	/// Executes `f` while recording dependencies for `id`,
	/// updating the recorded dependencies for `id` to the new set.
	///
	/// This process **may** cause subscription notification callbacks to be called.  
	/// Those callbacks **may or may not** happen before this method returns.
	///
	/// # Logic
	///
	/// Whenever calling this method causes removed dependencies to decome unsubscribed,
	/// their [`CallbackTable::on_subscribed_change`] callback **should** be invoked semantically
	/// *after* they have been removed as dependency of the signal identified by `id`.  
	/// (This avoids unnecessary invalidation of the latter.)
	///
	/// # Panics
	///
	/// This function **may** panic iff `id` is not started.
	///
	/// # See also
	///
	/// [`SignalsRuntimeRef::purge`]
	fn update_dependency_set<T>(&self, id: Self::Symbol, f: impl FnOnce() -> T) -> T;

	/// Increases the intrinsic subscription count of `id`.
	///
	/// An intrinsic subscription is one that is active regardless of dependents.
	///
	/// # Logic
	///
	/// This function **must** be callable at any time with any valid `id`.
	///
	/// # See also
	///
	/// [`SignalsRuntimeRef::purge`]
	fn subscribe(&self, id: Self::Symbol);

	/// Decreases the intrinsic subscription count of `id`.
	///
	/// An intrinsic subscription is one that is active regardless of dependents.
	///
	/// # Logic
	///
	/// If the [`CallbackTable::on_subscribed_change`] returns [`Propagation::FlushOut`],
	/// that **should** still cause refreshes of the unsubscribing dependencies (except
	/// for dependencies that have in fact been removed). This ensures that e.g. reference-
	/// counted resources can be freed appropriately. Such refreshes **may** be deferred.
	///
	/// This function **must** be callable at any time with any valid `id`.
	///
	/// # Panics
	///
	/// This function **should** panic iff the intrinsic subscription count falls below zero.
	///
	/// # Logic
	///
	/// However, the runtime **may** (but **should not**) avoid tracking this separately
	/// and instead exhibit unexpected behaviour iff there wasn't an at-least-equal number
	/// of [`subscribe`](`SignalsRuntimeRef::subscribe`) calls with the same `id`.
	///
	/// Attempting to decrease the net number of intrinsic subscriptions below zero
	/// **may** cause unexpected behaviour (but not undefined behaviour).
	///
	/// Note that [`purge`](`SignalsRuntimeRef::purge`) is expected to reset the net subscription count to zero.
	///
	/// # See also
	///
	/// [`SignalsRuntimeRef::purge`]
	fn unsubscribe(&self, id: Self::Symbol);

	/// Submits `f` to run exclusively for `id` *without* recording dependencies.
	///
	/// The runtime **should** run `f` eventually, but **may** cancel it in response to
	/// a [`.stop(id)`](`SignalsRuntimeRef::stop`) call with the same `id`.
	///
	/// # Panics
	///
	/// This function **may** panic unless called between [`.start`](`SignalsRuntimeRef::start`) and [`.stop`](`SignalsRuntimeRef::stop`) for `id`.
	///
	/// # Safety
	///
	/// `f` **must** be dropped or consumed before the next matching [`stop`](`SignalsRuntimeRef::stop`) call returns.
	fn update_or_enqueue(&self, id: Self::Symbol, f: impl 'static + Send + FnOnce() -> Propagation);

	/// **Immediately** submits `f` to run exclusively for `id` *without* recording dependencies.
	///
	/// Dropping the resulting [`Future`] cancels the scheduled update iff possible.
	///
	/// # Logic
	///
	/// The runtime **should** run `f` eventually, but **may** instead cancel and return it inside
	/// [`Err`] in response to a [`stop`](`SignalsRuntimeRef::stop`) call with the same `id`.
	///
	/// This method **must not** block indefinitely *as long as `f` doesn't*, regardless of context.  
	/// Calling [`stop`](`SignalsRuntimeRef::stop`) with matching `id` **should** cancel the update and return the [`Err`] variant.
	///
	/// # Safety
	///
	/// `f` **must not** run or be dropped after the next matching [`stop`](`SignalsRuntimeRef::stop`) call returns.  
	/// `f` **must not** run or be dropped after the [`Future`] returned by this function is dropped.
	fn update_eager<'f, T: 'f + Send, F: 'f + Send + FnOnce() -> (Propagation, T)>(
		&self,
		id: Self::Symbol,
		f: F,
	) -> Self::UpdateEager<'f, T, F>;

	/// The type of the [`Future`] returned by [`update_eager`](`SignalsRuntimeRef::update_eager`).
	///
	/// Dropping this [`Future`] **should** cancel the scheduled update if possible.
	type UpdateEager<'f, T: 'f, F: 'f>: 'f + Send + Future<Output = Result<T, F>>;

	/// Runs `f` exclusively for `id` *without* recording dependencies.
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
	///
	/// # Safety
	///
	/// `f` **must** be consumed before this method returns.
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
	/// includes notifying `id` of the subscription change from its intrinsic
	/// subscriptions being removed, where applicable.  
	/// The runtime **must not** indefinitely hold onto resources associated with `id`
	/// after this method returns.
	///
	/// The caller **may** reuse `id` later on as if fresh.
	///
	/// # Safety
	///
	/// [`purge`](`SignalsRuntimeRef::purge`) implies [`stop`](`SignalsRuntimeRef::stop`).
	fn purge(&self, id: Self::Symbol);

	/// Hints to the signals runtime that contained operations (usually: on the current thread)
	/// are related and that update propagation is likely to benefit from batching/deduplication.
	///
	/// Note that the runtime **may** ignore this completely.
	///
	/// # Logic
	///
	/// This function **may** act as "exclusivity context" for nested calls to [`update_blocking`](`SignalsRuntimeRef::update_blocking`),
	/// causing them to deadlock or panic.
	#[inline(always)]
	fn hint_batched_updates<T>(&self, f: impl FnOnce() -> T) -> T {
		f()
	}
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
/// This runtime is guaranteed to have settled whenever the *across all threads* last borrow
/// of it ceases, but only regarding effects originating on the current thread. Effects from
/// other threads won't necessarily be visible without external synchronisation points.
///
/// (This means that in addition to transiently borrowing calls, returned [`Future`]s
/// **may** cause the [`GlobalSignalsRuntime`] not to settle until they are dropped.)
///
/// Otherwise, it makes no additional guarantees over those specified in [`SignalsRuntimeRef`]'s documentation.
///
/// # Panics
///
/// [`SignalsRuntimeRef::Symbol`]s associated with the [`GlobalSignalsRuntime`] are ordered.  
/// Given [`GSRSymbol`]s `a` and `b`, `b` can depend on `a` only iff `a` < `b` (by creation order).
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlobalSignalsRuntime;

impl Debug for GlobalSignalsRuntime {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		if cfg!(feature = "global_signals_runtime") {
			#[cfg(feature = "global_signals_runtime")]
			Debug::fmt(&ISOPRENOID_GLOBAL_SIGNALS_RUNTIME, f)?;
			Ok(())
		} else {
			struct Unavailable;
			impl Debug for Unavailable {
				fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
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

/// A [`SignalsRuntimeRef::Symbol`] associated with the [`GlobalSignalsRuntime`].
///
/// Given [`GSRSymbol`]s `a` and `b`, `b` can depend on `a` only iff `a` < `b` (by creation order).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GSRSymbol(pub(crate) ASymbol);

impl Debug for GSRSymbol {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		f.debug_tuple("GSRSymbol").field(&self.0 .0).finish()
	}
}

mod global_callback_table_types {
	use super::ACallbackTableTypes;

	#[allow(unreachable_pub)]
	#[repr(transparent)]
	pub struct GlobalCallbackTableTypes(ACallbackTableTypes);
}
use global_callback_table_types::GlobalCallbackTableTypes;

impl CallbackTableTypes for GlobalCallbackTableTypes {
	//SAFETY: Everything here must be the same as for `ACallbackTableTypes`!
	type SubscribedStatus = bool;
}

#[cfg(feature = "global_signals_runtime")]
/// **The feature `"global_signals_runtime"` is required to enable this implementation.**
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

	fn subscribe(&self, id: Self::Symbol) {
		(&ISOPRENOID_GLOBAL_SIGNALS_RUNTIME).subscribe(id.0)
	}

	fn unsubscribe(&self, id: Self::Symbol) {
		(&ISOPRENOID_GLOBAL_SIGNALS_RUNTIME).unsubscribe(id.0)
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

	fn hint_batched_updates<T>(&self, f: impl FnOnce() -> T) -> T {
		(&ISOPRENOID_GLOBAL_SIGNALS_RUNTIME).hint_batched_updates(f)
	}
}

/// The `unsafe` at-runtime version of [`Callbacks`](`crate::raw::Callbacks`),
/// mainly for use between [`RawSignal`](`crate::raw::RawSignal`) and [`SignalsRuntimeRef`].
///
/// # Safety
///
/// The function pointers in this type may only be used as documented on [`SignalsRuntimeRef`].
#[repr(C)]
#[non_exhaustive]
pub struct CallbackTable<T: ?Sized, CTT: ?Sized + CallbackTableTypes> {
	/// A callback used to refresh stale signals.
	///
	/// Signals that are not currently subscribed **should** *outside of explicit flushing* **not** be refreshed *by the runtime*.  
	/// Signals **should** return only fresh values.  
	/// Signals **may** remain stale indefinitely.  
	/// Signals **may** be destroyed while stale.
	///
	/// # Logic
	///
	/// The runtime **must** record dependencies for this callback and update them afterwards.
	pub update: Option<unsafe fn(*const T) -> Propagation>,

	/// A callback used to notify a signal of a change in its subscribed-state.
	///
	/// This is separate from the automatic refresh applied to stale signals that become subscribed to.
	///
	/// # Logic
	///
	/// The runtime **must** consider transitive subscriptions.  
	/// The runtime **must** consider a signal's own intrinsic subscriptions.  
	/// The runtime **must not** run this function while recording dependencies (but may start a nested recording in response to the callback).
	pub on_subscribed_change:
		Option<unsafe fn(*const T, status: CTT::SubscribedStatus) -> Propagation>,
}

impl<T: ?Sized, CTT: ?Sized + CallbackTableTypes> Debug for CallbackTable<T, CTT> {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
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
	#[allow(unpredictable_function_pointer_comparisons)] // Used only for interning.
	fn eq(&self, other: &Self) -> bool {
		self.update == other.update && self.on_subscribed_change == other.on_subscribed_change
	}
}

impl<T: ?Sized, CTT: ?Sized + CallbackTableTypes> Eq for CallbackTable<T, CTT> {}

impl<T: ?Sized, CTT: ?Sized + CallbackTableTypes> PartialOrd for CallbackTable<T, CTT> {
	#[allow(unpredictable_function_pointer_comparisons)] // Used only for interning.
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
	#[allow(unpredictable_function_pointer_comparisons)] // Used only for interning.
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
	/// "Type-erases" the pointed-to callback table against the data type `T` by replacing it with `()` in the signature.
	///
	/// Note that the callback functions still may only be called using the originally correct data pointer(s).
	pub fn into_erased_ptr(this: *const Self) -> *const CallbackTable<(), CTT> {
		this.cast()
	}

	/// "Type-erases" the pointed-to callback table against the data type `T` by replacing it with `()` in the signature.
	///
	/// Note that the callback functions still may only be called using the originally correct data pointer(s).
	pub fn into_erased(self) -> CallbackTable<(), CTT> {
		unsafe { mem::transmute(self) }
	}
}

/// A return value used by [`CallbackTable`]/[`Callbacks`](`crate::raw::Callbacks`) callbacks
/// to indicate whether to flag dependent signals as stale and optionally also refresh ones not currently subscribed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[must_use = "The runtime should propagate notifications to dependents only when requested."]
pub enum Propagation {
	/// Mark at least directly dependent signals as stale.  
	/// The runtime decides whether and when to refresh them.
	Propagate,
	/// Do not mark dependent signals as stale because of this [`Propagation`].
	Halt,
	/// Asks the runtime to refresh dependencies, even those that are not subscribed.
	///
	/// This **should** be transitive through [`Propagate`](`Propagation::Propagate`) of dependents,  
	/// but **should not** be transitive through [`Halt`](`Propagation::Halt`).
	///
	/// > **Hint**
	/// >
	/// > Use this variant to purge heavy or reference-counted resources store in dependent signals.
	FlushOut,
}

mod private {
	use std::{
		future::Future,
		pin::Pin,
		task::{Context, Poll},
	};

	use futures_lite::FutureExt;

	#[allow(unreachable_pub)] // Used with "global_signals_runtime".
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
