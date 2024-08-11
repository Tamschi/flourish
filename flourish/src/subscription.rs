use std::{
	borrow::Borrow,
	fmt::{self, Debug, Formatter},
	marker::PhantomData,
	mem::ManuallyDrop,
	ops::Deref,
	pin::Pin,
};

use isoprenoid::runtime::{GlobalSignalsRuntime, Propagation, SignalsRuntimeRef};

use crate::{
	opaque::Opaque,
	signal::Strong,
	traits::{Guard, Subscribable},
	unmanaged::{computed, folded, reduced},
	Signal, SignalArc,
};

/// Type inference helper alias for [`SubscriptionSR`] (using [`GlobalSignalsRuntime`]).
pub type SubscriptionArc_<T, S> = Subscription<T, S, GlobalSignalsRuntime>;

/// Type of [`SubscriptionSR`]s after type-erasure. Dynamic dispatch.
pub type SubscriptionArcDyn<'a, T, SR> = Subscription<T, dyn 'a + Subscribable<T, SR>, SR>;

/// Intrinsically-subscribed version of [`SignalSR`].  
/// Can be directly constructed but also converted to and from that type.
#[must_use = "Subscriptions are cancelled when dropped."]
pub struct Subscription<
	T: ?Sized + Send,
	S: ?Sized + Subscribable<T, SR>,
	SR: ?Sized + SignalsRuntimeRef,
> {
	pub(crate) subscribed: Strong<T, S, SR>,
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Deref
	for Subscription<T, S, SR>
{
	type Target = Signal<T, S, SR>;

	fn deref(&self) -> &Self::Target {
		&self.subscribed
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Borrow<Signal<T, S, SR>> for Subscription<T, S, SR>
{
	fn borrow(&self) -> &Signal<T, S, SR> {
		self.subscribed.borrow()
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Debug
	for Subscription<T, S, SR>
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
	for Subscription<T, S, SR>
{
}
unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Sync
	for Subscription<T, S, SR>
{
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Drop
	for Subscription<T, S, SR>
{
	fn drop(&mut self) {
		self.source.as_ref().unsubscribe();
	}
}

impl<
		'a,
		T: 'a + ?Sized + Send,
		S: 'a + Sized + Subscribable<T, SR>,
		SR: 'a + ?Sized + SignalsRuntimeRef,
	> From<Subscription<T, S, SR>> for SubscriptionArcDyn<'a, T, SR>
{
	fn from(value: Subscription<T, S, SR>) -> Self {
		value.into_dyn()
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: SignalsRuntimeRef>
	Subscription<T, S, SR>
{
	/// Constructs a new [`SubscriptionSR`] from the given "raw" [`Subscribable`].
	///
	/// The subscribable is subscribed-to intrinsically.
	///
	/// # Panics
	///
	/// Iff the call to [`Subscribable::subscribe`] fails. This should never happen, as
	/// the subscribable shouldn't have been in a state where it could be subscribed to
	/// before pinning.
	pub fn new(source: S) -> Self
	where
		S: Sized,
	{
		source.clone_runtime_ref().run_detached(|| {
			let strong = Strong::pin(source);
			strong._managed().subscribe();
			Self { subscribed: strong }
		})
	}

	/// Unsubscribes the [`SubscriptionSR`], turning it into a [`SignalSR`] in the process.
	///
	/// The underlying [`Source`](`crate::raw::Source`) may remain effectively subscribed due to subscribed dependencies.
	#[must_use = "Use `drop(self)` instead of converting first. The effect is the same."]
	pub fn unsubscribe(self) -> SignalArc<T, S, SR> {
		//FIXME: This could avoid refcounting up and down and the associated memory barriers.
		self.to_signal()
	} // Implicit drop(self) unsubscribes.

	/// Cheaply clones this handle into a [`SignalSR`].
	pub fn to_signal(self) -> SignalArc<T, S, SR> {
		SignalArc {
			strong: self.subscribed.clone(),
		}
	}
}

impl<T: ?Sized + Send, S: Sized + Subscribable<T, SR>, SR: SignalsRuntimeRef>
	Subscription<T, S, SR>
{
	pub fn into_dyn<'a>(self) -> SubscriptionArcDyn<'a, T, SR>
	where
		T: 'a,
		S: 'a,
		SR: 'a,
	{
		let this = ManuallyDrop::new(self);

		let dyn_ = SubscriptionArcDyn {
			source: Pin::clone(&this.source) as Pin<Arc<_>>,
			_phantom: PhantomData,
		};

		unsafe {
			let ptr = Arc::into_raw(Pin::into_inner_unchecked(Pin::clone(&this.source)));
			Arc::decrement_strong_count(ptr);
			Arc::decrement_strong_count(ptr);
		}

		dyn_
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
/// let sub = Signal::computed_uncached(|| ()).subscribe();
/// # }
/// ```
impl<T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef> Subscription<T, Opaque, SR> {
	/// A simple cached computation.
	///
	/// Wraps [`computed`](`computed()`).
	pub fn computed<'a>(
		fn_pin: impl 'a + Send + FnMut() -> T,
	) -> Subscription<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a + Default,
	{
		Subscription::new(computed(fn_pin, SR::default()))
	}

	/// A simple cached computation.
	///
	/// Wraps [`computed`](`computed()`).
	pub fn computed_with_runtime<'a>(
		fn_pin: impl 'a + Send + FnMut() -> T,
		runtime: SR,
	) -> Subscription<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a,
	{
		Subscription::new(computed(fn_pin, runtime))
	}

	/// The closure mutates the value and can choose to [`Halt`](`Update::Halt`) propagation.
	///
	/// Wraps [`folded`](`folded()`).
	pub fn folded<'a>(
		init: T,
		fold_fn_pin: impl 'a + Send + FnMut(&mut T) -> Propagation,
	) -> Subscription<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a + Default,
	{
		Subscription::new(folded(init, fold_fn_pin, SR::default()))
	}

	/// The closure mutates the value and can choose to [`Halt`](`Update::Halt`) propagation.
	///
	/// Wraps [`folded`](`folded()`).
	pub fn folded_with_runtime<'a>(
		init: T,
		fold_fn_pin: impl 'a + Send + FnMut(&mut T) -> Propagation,
		runtime: SR,
	) -> Subscription<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a,
	{
		Subscription::new(folded(init, fold_fn_pin, runtime))
	}

	/// `select_fn_pin` computes each value, `reduce_fn_pin` updates current with next and can choose to [`Halt`](`Update::Halt`) propagation.
	/// Dependencies are detected across both closures.
	///
	/// Wraps [`reduced`](`reduced()`).
	pub fn reduced<'a>(
		select_fn_pin: impl 'a + Send + FnMut() -> T,
		reduce_fn_pin: impl 'a + Send + FnMut(&mut T, T) -> Propagation,
	) -> Subscription<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a + Default,
	{
		Subscription::new(reduced(select_fn_pin, reduce_fn_pin, SR::default()))
	}

	/// `select_fn_pin` computes each value, `reduce_fn_pin` updates current with next and can choose to [`Halt`](`Update::Halt`) propagation.
	/// Dependencies are detected across both closures.
	///
	/// Wraps [`reduced`](`reduced()`).
	pub fn reduced_with_runtime<'a>(
		select_fn_pin: impl 'a + Send + FnMut() -> T,
		reduce_fn_pin: impl 'a + Send + FnMut(&mut T, T) -> Propagation,
		runtime: SR,
	) -> Subscription<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a,
	{
		Subscription::new(reduced(select_fn_pin, reduce_fn_pin, runtime))
	}
}

/// ⚠️ This implementation uses dynamic dispatch internally for all methods with `Self: Sized`
/// bound, which is a bit less performant than using those accessors without type erasure.
impl<'a, T: 'a + ?Sized + Send, SR: 'a + ?Sized + SignalsRuntimeRef> SourcePin<T, SR>
	for SubscriptionArcDyn<'a, T, SR>
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

	fn read<'r>(&'r self) -> private::BoxedGuardDyn<'r, T>
	where
		Self: Sized,
		T: 'r + Sync,
	{
		private::BoxedGuardDyn(self.source.as_ref().read_dyn())
	}

	type Read<'r> = private::BoxedGuardDyn<'r, T>
	where
		Self: 'r + Sized,
		T: 'r + Sync;

	fn read_exclusive<'r>(&'r self) -> private::BoxedGuardDyn<'r, T>
	where
		Self: Sized,
		T: 'r,
	{
		private::BoxedGuardDyn(self.source.as_ref().read_exclusive_dyn())
	}

	type ReadExclusive<'r> = private::BoxedGuardDyn<'r, T>
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

/// Duplicated to avoid identities.
mod private {
	use std::{borrow::Borrow, ops::Deref};

	use crate::traits::Guard;

	pub struct BoxedGuardDyn<'r, T: ?Sized>(pub(super) Box<dyn 'r + Guard<T>>);

	impl<T: ?Sized> Guard<T> for BoxedGuardDyn<'_, T> {}

	impl<T: ?Sized> Deref for BoxedGuardDyn<'_, T> {
		type Target = T;

		fn deref(&self) -> &Self::Target {
			self.0.deref()
		}
	}

	impl<T: ?Sized> Borrow<T> for BoxedGuardDyn<'_, T> {
		fn borrow(&self) -> &T {
			(*self.0).borrow()
		}
	}
}
