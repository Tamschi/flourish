use std::{
	fmt::{self, Debug, Formatter},
	marker::PhantomData,
	pin::Pin,
	sync::{Arc, Weak},
};

use isoprenoid::runtime::{GlobalSignalsRuntime, Propagation, SignalsRuntimeRef};

use crate::{
	opaque::Opaque,
	raw::{computed, folded, reduced},
	traits::{Guard, Subscribable},
	SignalRef, SignalSR, SourcePin,
};

/// Type inference helper alias for [`SubscriptionSR`] (using [`GlobalSignalsRuntime`]).
pub type Subscription<T, S> = SubscriptionSR<T, S, GlobalSignalsRuntime>;

/// Type of [`SubscriptionSR`]s after type-erasure. Dynamic dispatch.
pub type SubscriptionDyn<'a, T, SR> = SubscriptionSR<T, dyn 'a + Subscribable<T, SR>, SR>;

pub type WeakSubscriptionDyn<'a, T, SR> = WeakSubscription<T, dyn 'a + Subscribable<T, SR>, SR>;

pub struct WeakSubscription<
	T: ?Sized + Send,
	S: ?Sized + Subscribable<T, SR>,
	SR: ?Sized + SignalsRuntimeRef,
> {
	source_cell: Weak<S>,
	_phantom: PhantomData<(PhantomData<T>, SR)>,
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	WeakSubscription<T, S, SR>
{
	#[must_use]
	pub fn upgrade(&self) -> Option<SignalSR<T, S, SR>> {
		self.source_cell.upgrade().map(|strong| SignalSR {
			source: unsafe { Pin::new_unchecked(strong) },
			_phantom: PhantomData,
		})
	}
}

/// Inherently-subscribed version of [`SignalSR`].  
/// Can be directly constructed but also converted to and from that type.
#[must_use = "Subscriptions are cancelled when dropped."]
pub struct SubscriptionSR<
	T: ?Sized + Send,
	S: ?Sized + Subscribable<T, SR>,
	SR: ?Sized + SignalsRuntimeRef,
> {
	pub(crate) source: Pin<Arc<S>>,
	pub(crate) _phantom: PhantomData<(PhantomData<T>, SR)>,
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Debug
	for SubscriptionSR<T, S, SR>
where
	T: Debug,
{
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		self.source.clone_runtime_ref().run_detached(|| {
			f.debug_struct("SubscriptionSR")
				.field("(value)", &&**self.source.as_ref().read_exclusive_dyn())
				.finish_non_exhaustive()
		})
	}
}

unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Send
	for SubscriptionSR<T, S, SR>
{
}
unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Sync
	for SubscriptionSR<T, S, SR>
{
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Drop
	for SubscriptionSR<T, S, SR>
{
	fn drop(&mut self) {
		self.source.as_ref().unsubscribe_inherently();
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: SignalsRuntimeRef>
	SubscriptionSR<T, S, SR>
{
	/// Constructs a new [`SubscriptionSR`] from the given "raw" [`Subscribable`].
	///
	/// The subscribable is subscribed-to inherently.
	///
	/// # Panics
	///
	/// Iff the call to [`Subscribable::subscribe_inherently`] fails. This should never happen,
	/// as the subscribable shouldn't have been in a state where it could be subscribed to before pinning.
	pub fn new(source: S) -> Self
	where
		S: Sized,
	{
		source.clone_runtime_ref().run_detached(|| {
			let arc = Arc::pin(source);
			assert!(
				arc.as_ref().subscribe_inherently(),
				"Couldn't subscribe to the subscribable."
			);
			Self {
				source: arc,
				_phantom: PhantomData,
			}
		})
	}

	/// Cheaply borrows this [`SubscriptionSR`] as [`SignalRef`], which is [`Copy`].
	pub fn as_ref(&self) -> SignalRef<'_, T, S, SR> {
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
	pub fn unsubscribe(self) -> SignalSR<T, S, SR> {
		//FIXME: This could avoid refcounting up and down and the associated memory barriers.
		SignalSR {
			source: Pin::clone(&self.source),
			_phantom: PhantomData,
		}
	} // Implicit drop(self) unsubscribes.

	/// Cheaply clones this handle into a [`SignalSR`].
	///
	/// Only one handle can own the inherent subscription of the managed [`Subscribable`].
	#[must_use = "Pure function."]
	pub fn to_signal(self) -> SignalSR<T, S, SR> {
		SignalSR {
			source: Pin::clone(&self.source),
			_phantom: PhantomData,
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
/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
/// use flourish::Signal;
///
/// // The closure runs once on subscription, but not to refresh `sub`!
/// // It re-runs with each access of its value through `SourcePin`, instead.
/// let sub = Signal::computed_uncached(|| ())
///     .try_subscribe()
///     .expect("contextually infallible");
/// # }
/// ```
impl<T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef> SubscriptionSR<T, Opaque, SR> {
	/// A simple cached computation.
	///
	/// Wraps [`computed`](`computed()`).
	pub fn computed<'a>(
		fn_pin: impl 'a + Send + FnMut() -> T,
	) -> SubscriptionSR<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a + Default,
	{
		SubscriptionSR::new(computed(fn_pin, SR::default()))
	}

	/// A simple cached computation.
	///
	/// Wraps [`computed`](`computed()`).
	pub fn computed_with_runtime<'a>(
		fn_pin: impl 'a + Send + FnMut() -> T,
		runtime: SR,
	) -> SubscriptionSR<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a,
	{
		SubscriptionSR::new(computed(fn_pin, runtime))
	}

	/// The closure mutates the value and can choose to [`Halt`](`Update::Halt`) propagation.
	///
	/// Wraps [`folded`](`folded()`).
	pub fn folded<'a>(
		init: T,
		fold_fn_pin: impl 'a + Send + FnMut(&mut T) -> Propagation,
	) -> SubscriptionSR<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a + Default,
	{
		SubscriptionSR::new(folded(init, fold_fn_pin, SR::default()))
	}

	/// The closure mutates the value and can choose to [`Halt`](`Update::Halt`) propagation.
	///
	/// Wraps [`folded`](`folded()`).
	pub fn folded_with_runtime<'a>(
		init: T,
		fold_fn_pin: impl 'a + Send + FnMut(&mut T) -> Propagation,
		runtime: SR,
	) -> SubscriptionSR<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a,
	{
		SubscriptionSR::new(folded(init, fold_fn_pin, runtime))
	}

	/// `select_fn_pin` computes each value, `reduce_fn_pin` updates current with next and can choose to [`Halt`](`Update::Halt`) propagation.
	/// Dependencies are detected across both closures.
	///
	/// Wraps [`reduced`](`reduced()`).
	pub fn reduced<'a>(
		select_fn_pin: impl 'a + Send + FnMut() -> T,
		reduce_fn_pin: impl 'a + Send + FnMut(&mut T, T) -> Propagation,
	) -> SubscriptionSR<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a + Default,
	{
		SubscriptionSR::new(reduced(select_fn_pin, reduce_fn_pin, SR::default()))
	}

	/// `select_fn_pin` computes each value, `reduce_fn_pin` updates current with next and can choose to [`Halt`](`Update::Halt`) propagation.
	/// Dependencies are detected across both closures.
	///
	/// Wraps [`reduced`](`reduced()`).
	pub fn reduced_with_runtime<'a>(
		select_fn_pin: impl 'a + Send + FnMut() -> T,
		reduce_fn_pin: impl 'a + Send + FnMut(&mut T, T) -> Propagation,
		runtime: SR,
	) -> SubscriptionSR<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a,
	{
		SubscriptionSR::new(reduced(select_fn_pin, reduce_fn_pin, runtime))
	}
}

impl<T: ?Sized + Send, S: Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	SourcePin<T, SR> for SubscriptionSR<T, S, SR>
{
	fn touch(&self) {
		self.source.as_ref().touch()
	}

	fn get_clone(&self) -> T
	where
		T: Sync + Clone,
	{
		self.source.as_ref().get_clone()
	}

	fn get_clone_exclusive(&self) -> T
	where
		T: Clone,
	{
		self.source.as_ref().get_clone_exclusive()
	}

	fn read<'r>(&'r self) -> S::Read<'r>
	where
		Self: Sized,
		T: 'r + Sync,
	{
		self.source.as_ref().read()
	}

	type Read<'r> = S::Read<'r>
	where
		Self: 'r + Sized,
		T: 'r + Sync;

	fn read_exclusive<'r>(&'r self) -> S::ReadExclusive<'r>
	where
		Self: Sized,
		T: 'r,
	{
		self.source.as_ref().read_exclusive()
	}

	type ReadExclusive<'r> = S::ReadExclusive<'r>
	where
		Self: 'r + Sized,
		T: 'r;

	fn read_dyn<'r>(&'r self) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r + Sync,
	{
		self.source.as_ref().read_dyn()
	}

	fn read_exclusive_dyn<'r>(&'r self) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r,
	{
		self.source.as_ref().read_exclusive_dyn()
	}

	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		self.source.as_ref().clone_runtime_ref()
	}
}
