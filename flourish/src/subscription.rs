use std::{
	borrow::Borrow,
	fmt::{self, Debug, Formatter},
	marker::PhantomData,
	pin::Pin,
	sync::Arc,
};

use isoprenoid::runtime::{GlobalSignalsRuntime, Propagation, SignalsRuntimeRef};

use crate::{
	raw::{computed, folded, reduced},
	traits::Subscribable,
	SignalRef, SignalSR, SourcePin,
};

/// Type inference helper alias for [`SubscriptionSR`] (using [`GlobalSignalsRuntime`]).
pub type Subscription<'a, T> = SubscriptionSR<'a, T, GlobalSignalsRuntime>;

/// Inherently-subscribed version of [`SignalSR`].  
/// Can be directly constructed but also converted to and fro.
#[must_use = "Subscriptions are cancelled when dropped."]
pub struct SubscriptionSR<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalsRuntimeRef> {
	pub(crate) source: Pin<Arc<dyn 'a + Subscribable<SR, Output = T>>>,
}

impl<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalsRuntimeRef> Debug
	for SubscriptionSR<'a, T, SR>
where
	T: Debug,
{
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		self.source.clone_runtime_ref().run_detached(|| {
			f.debug_struct("SubscriptionSR")
				.field(
					"(value)",
					&(&*self.source.as_ref().read_exclusive()).borrow(),
				)
				.finish_non_exhaustive()
		})
	}
}

unsafe impl<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalsRuntimeRef> Send
	for SubscriptionSR<'a, T, SR>
{
}
unsafe impl<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalsRuntimeRef> Sync
	for SubscriptionSR<'a, T, SR>
{
}

impl<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalsRuntimeRef> Drop
	for SubscriptionSR<'a, T, SR>
{
	fn drop(&mut self) {
		self.source.as_ref().unsubscribe_inherently();
	}
}

impl<'a, T: 'a + Send + ?Sized, SR: SignalsRuntimeRef> SubscriptionSR<'a, T, SR> {
	/// Constructs a new [`SubscriptionSR`] from the given "raw" [`Subscribable`].
	///
	/// The subscribable is subscribed-to inherently.
	///
	/// # Panics
	///
	/// Iff the call to [`Subscribable::subscribe_inherently`] fails. This should never happen,
	/// as the subscribable shouldn't have been in a state where it could be subscribed to before pinning.
	pub fn new<S: 'a + Subscribable<SR, Output = T>>(source: S) -> Self {
		source.clone_runtime_ref().run_detached(|| {
			let arc = Arc::pin(source);
			arc.as_ref()
				.subscribe_inherently()
				.expect("Couldn't subscribe to the subscribable.");
			Self { source: arc }
		})
	}

	/// Cheaply borrows this [`SubscriptionSR`] as [`SignalRef`], which is [`Copy`].
	pub fn as_ref(&self) -> SignalRef<'_, 'a, T, SR> {
		SignalRef {
			source: {
				let ptr =
					Arc::into_raw(unsafe { Pin::into_inner_unchecked(Pin::clone(&self.source)) });
				unsafe { Arc::decrement_strong_count(ptr) };
				ptr
			},
			_phantom: PhantomData,
		}
	}

	/// Unsubscribes the [`SubscriptionSR`], turning it into a [`SignalSR`] in the process.
	///
	/// The underlying [`Source`](`crate::raw::Source`) may remain effectively subscribed due to subscribed dependencies.
	#[must_use = "Use `drop(self)` instead of converting first. The effect is the same."]
	pub fn unsubscribe(self) -> SignalSR<'a, T, SR> {
		//FIXME: This could avoid refcounting up and down and the associated memory barriers.
		SignalSR {
			source: Pin::clone(&self.source),
		}
	} // Implicit drop(self) unsubscribes.

	/// Cheaply clones this handle into a [`SignalSR`].
	///
	/// Only one handle can own the inherent subscription of the managed [`Subscribable`].
	#[must_use = "Pure function."]
	pub fn to_signal(self) -> SignalSR<'a, T, SR> {
		SignalSR {
			source: Pin::clone(&self.source),
		}
	}
}

/// Secondary constructors.
///
/// # Omissions
///
/// The "uncached" versions of [`computed`](`computed()`) are intentionally not wrapped here,
/// as their behaviour may be unexpected at first glance.
///
/// You can still easily construct them as [`SignalSR`] and subscribe afterwards:
///
/// ```
/// # {
/// #![cfg(feature = "global_signals_runtime")]
/// use flourish::Signal;
///
/// // The closure runs once on subscription, but not to refresh `sub`!
/// // It re-runs with each access of its value through `SourcePin`, instead.
/// let sub = Signal::computed_uncached(|| ())
///     .try_subscribe()
///     .expect("contextually infallible");
/// # }
/// ```
impl<'a, T: 'a + Send, SR: SignalsRuntimeRef> SubscriptionSR<'a, T, SR> {
	/// A simple cached computation.
	///
	/// Wraps [`computed`](`computed()`).
	pub fn computed(fn_pin: impl 'a + Send + FnMut() -> T) -> Self
	where
		SR: Default,
	{
		Self::new(computed(fn_pin, SR::default()))
	}

	/// A simple cached computation.
	///
	/// Wraps [`computed`](`computed()`).
	pub fn computed_with_runtime(fn_pin: impl 'a + Send + FnMut() -> T, runtime: SR) -> Self {
		Self::new(computed(fn_pin, runtime))
	}

	/// The closure mutates the value and can choose to [`Halt`](`Update::Halt`) propagation.
	///
	/// Wraps [`folded`](`folded()`).
	pub fn folded(init: T, fold_fn_pin: impl 'a + Send + FnMut(&mut T) -> Propagation) -> Self
	where
		SR: Default,
	{
		Self::new(folded(init, fold_fn_pin, SR::default()))
	}

	/// The closure mutates the value and can choose to [`Halt`](`Update::Halt`) propagation.
	///
	/// Wraps [`folded`](`folded()`).
	pub fn folded_with_runtime(
		init: T,
		fold_fn_pin: impl 'a + Send + FnMut(&mut T) -> Propagation,
		runtime: SR,
	) -> Self {
		Self::new(folded(init, fold_fn_pin, runtime))
	}

	/// `select_fn_pin` computes each value, `reduce_fn_pin` updates current with next and can choose to [`Halt`](`Update::Halt`) propagation.
	/// Dependencies are detected across both closures.
	///
	/// Wraps [`reduced`](`reduced()`).
	pub fn reduced(
		select_fn_pin: impl 'a + Send + FnMut() -> T,
		reduce_fn_pin: impl 'a + Send + FnMut(&mut T, T) -> Propagation,
	) -> Self
	where
		SR: Default,
	{
		Self::new(reduced(select_fn_pin, reduce_fn_pin, SR::default()))
	}

	/// `select_fn_pin` computes each value, `reduce_fn_pin` updates current with next and can choose to [`Halt`](`Update::Halt`) propagation.
	/// Dependencies are detected across both closures.
	///
	/// Wraps [`reduced`](`reduced()`).
	pub fn reduced_with_runtime(
		select_fn_pin: impl 'a + Send + FnMut() -> T,
		reduce_fn_pin: impl 'a + Send + FnMut(&mut T, T) -> Propagation,
		runtime: SR,
	) -> Self {
		Self::new(reduced(select_fn_pin, reduce_fn_pin, runtime))
	}
}

impl<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalsRuntimeRef> SourcePin<SR>
	for SubscriptionSR<'a, T, SR>
{
	type Output = T;

	fn touch(&self) {
		self.source.as_ref().touch()
	}

	fn get_clone(&self) -> Self::Output
	where
		Self::Output: Sync + Clone,
	{
		self.source.as_ref().get_clone()
	}

	fn get_clone_exclusive(&self) -> Self::Output
	where
		Self::Output: Clone,
	{
		self.source.as_ref().get_clone_exclusive()
	}

	fn read<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Output>>
	where
		Self::Output: 'r + Sync,
	{
		self.source.as_ref().read()
	}

	fn read_exclusive<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Output>> {
		self.source.as_ref().read_exclusive()
	}

	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		self.source.as_ref().clone_runtime_ref()
	}
}
