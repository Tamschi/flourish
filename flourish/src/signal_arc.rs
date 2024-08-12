use std::{
	borrow::Borrow,
	fmt::{self, Debug, Formatter},
	ops::Deref,
};

use isoprenoid::runtime::{Propagation, SignalsRuntimeRef};

use crate::{
	opaque::Opaque,
	signal::{Signal, Strong, Weak},
	traits::{Subscribable, UnmanagedSignalCell},
	unmanaged::{computed, computed_uncached, computed_uncached_mut, debounced, folded, reduced},
	Subscription,
};

/// Type of [`SignalSR`]s after type-erasure. Dynamic dispatch.
pub type SignalArcDyn<'a, T, SR> = SignalArc<T, dyn 'a + Subscribable<T, SR>, SR>;

/// Type of [`SignalWeak`]s after type-erasure or [`SignalDyn`] after downgrade. Dynamic dispatch.
pub type SignalWeakDyn<'a, T, SR> = SignalWeak<T, dyn 'a + Subscribable<T, SR>, SR>;

/// Type of [`SignalWeak`]s after cell-type-erasure or [`SignalDynCell`] after downgrade. Dynamic dispatch.
pub type SignalWeakDynCell<'a, T, SR> = SignalWeak<T, dyn 'a + UnmanagedSignalCell<T, SR>, SR>;

#[repr(transparent)]
pub struct SignalWeak<
	T: ?Sized + Send,
	S: ?Sized + Subscribable<T, SR>,
	SR: ?Sized + SignalsRuntimeRef,
> {
	pub(crate) weak: Weak<T, S, SR>,
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	SignalWeak<T, S, SR>
{
	#[must_use]
	pub fn upgrade(&self) -> Option<SignalArc<T, S, SR>> {
		self.weak.upgrade().map(|strong| SignalArc { strong })
	}
}

/// A largely type-erased signal handle that is all of [`Clone`], [`Send`], [`Sync`] and [`Unpin`].
///
/// To access values, import [`SourcePin`].
///
/// Signals are not evaluated unless they are subscribed-to (or on demand if if not current).  
/// Uncached signals are instead evaluated on direct demand **only** (but still communicate subscriptions and invalidation).
#[must_use = "Signals are generally inert unless subscribed to."]
pub struct SignalArc<
	T: ?Sized + Send,
	S: ?Sized + Subscribable<T, SR>,
	SR: ?Sized + SignalsRuntimeRef,
> {
	pub(super) strong: Strong<T, S, SR>,
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Clone
	for SignalArc<T, S, SR>
{
	fn clone(&self) -> Self {
		Self {
			strong: self.strong.clone(),
		}
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Debug
	for SignalArc<T, S, SR>
where
	T: Debug,
{
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		self.strong.clone_runtime_ref().run_detached(|| {
			f.debug_struct("SignalSR")
				.field("(value)", &&**self.source.as_ref().read_exclusive_dyn())
				.finish_non_exhaustive()
		})
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Deref
	for SignalArc<T, S, SR>
{
	type Target = Signal<T, S, SR>;

	fn deref(&self) -> &Self::Target {
		&self.strong
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Borrow<Signal<T, S, SR>> for SignalArc<T, S, SR>
{
	fn borrow(&self) -> &Signal<T, S, SR> {
		self.strong.borrow()
	}
}

unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Send
	for SignalArc<T, S, SR>
{
}
unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Sync
	for SignalArc<T, S, SR>
{
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	SignalArc<T, S, SR>
{
	/// Creates a new [`SignalSR`] from the provided raw [`Subscribable`].
	pub fn new(source: S) -> Self
	where
		S: Sized,
	{
		SignalArc {
			strong: Strong::pin(source),
		}
	}

	//TODO: Various `From` and `TryFrom` conversions, including for unsizing.

	pub fn subscribe(self) -> Subscription<T, S, SR> {
		self.source.as_ref().subscribe();
		Subscription {
			subscribed: self.strong.clone(),
		}
	}

	pub fn into_dyn<'a>(self) -> SignalArcDyn<'a, T, SR>
	where
		S: 'a + Sized,
	{
		let Self { strong } = self;
		SignalArcDyn { strong }
	}
}

/// Secondary constructors.
impl<T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef> SignalArc<T, Opaque, SR> {
	/// A simple cached computation.
	///
	/// Wraps [`computed`](`computed()`).
	pub fn computed<'a>(
		fn_pin: impl 'a + Send + FnMut() -> T,
	) -> SignalArc<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a + Default,
	{
		Self::computed_with_runtime(fn_pin, SR::default())
	}

	/// A simple cached computation.
	///
	/// Wraps [`computed`](`computed()`).
	pub fn computed_with_runtime<'a>(
		fn_pin: impl 'a + Send + FnMut() -> T,
		runtime: SR,
	) -> SignalArc<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a,
	{
		SignalArc::new(computed(fn_pin, runtime))
	}

	/// A simple cached computation.
	///
	/// Doesn't update its cache or propagate iff the new result is equal.
	///
	/// Wraps [`debounced`](`debounced()`).
	pub fn debounced<'a>(
		fn_pin: impl 'a + Send + FnMut() -> T,
	) -> SignalArc<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized + PartialEq,
		SR: 'a + Default,
	{
		Self::debounced_with_runtime(fn_pin, SR::default())
	}

	/// A simple cached computation.
	///
	/// Doesn't update its cache or propagate iff the new result is equal.
	///
	/// Wraps [`debounced`](`debounced()`).
	pub fn debounced_with_runtime<'a>(
		fn_pin: impl 'a + Send + FnMut() -> T,
		runtime: SR,
	) -> SignalArc<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized + PartialEq,
		SR: 'a,
	{
		SignalArc::new(debounced(fn_pin, runtime))
	}

	/// A simple **uncached** computation.
	///
	/// Wraps [`computed_uncached`](`computed_uncached()`).
	pub fn computed_uncached<'a>(
		fn_pin: impl 'a + Send + Sync + Fn() -> T,
	) -> SignalArc<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a + Default,
	{
		Self::computed_uncached_with_runtime(fn_pin, SR::default())
	}

	/// A simple **uncached** computation.
	///
	/// Wraps [`computed_uncached`](`computed_uncached()`).
	pub fn computed_uncached_with_runtime<'a>(
		fn_pin: impl 'a + Send + Sync + Fn() -> T,
		runtime: SR,
	) -> SignalArc<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a,
	{
		SignalArc::new(computed_uncached(fn_pin, runtime))
	}

	/// A simple **stateful uncached** computation.
	///
	/// ⚠️ Care must be taken to avoid unexpected behaviour!
	///
	/// Wraps [`computed_uncached_mut`](`computed_uncached_mut()`).
	pub fn computed_uncached_mut<'a>(
		fn_pin: impl 'a + Send + FnMut() -> T,
	) -> SignalArc<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a + Default,
	{
		Self::computed_uncached_mut_with_runtime(fn_pin, SR::default())
	}

	/// A simple **stateful uncached** computation.
	///
	/// ⚠️ Care must be taken to avoid unexpected behaviour!
	///
	/// Wraps [`computed_uncached_mut`](`computed_uncached_mut()`).
	pub fn computed_uncached_mut_with_runtime<'a>(
		fn_pin: impl 'a + Send + FnMut() -> T,
		runtime: SR,
	) -> SignalArc<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a,
	{
		SignalArc::new(computed_uncached_mut(fn_pin, runtime))
	}

	/// The closure mutates the value and can choose to [`Halt`](`Update::Halt`) propagation.
	///
	/// Wraps [`folded`](`folded()`).
	pub fn folded<'a>(
		init: T,
		fold_fn_pin: impl 'a + Send + FnMut(&mut T) -> Propagation,
	) -> SignalArc<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a + Default,
	{
		Self::folded_with_runtime(init, fold_fn_pin, SR::default())
	}

	/// The closure mutates the value and can choose to [`Halt`](`Update::Halt`) propagation.
	///
	/// Wraps [`folded`](`folded()`).
	pub fn folded_with_runtime<'a>(
		init: T,
		fold_fn_pin: impl 'a + Send + FnMut(&mut T) -> Propagation,
		runtime: SR,
	) -> SignalArc<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a,
	{
		SignalArc::new(folded(init, fold_fn_pin, runtime))
	}

	/// `select_fn_pin` computes each value, `reduce_fn_pin` updates current with next and can choose to [`Halt`](`Update::Halt`) propagation.
	/// Dependencies are detected across both closures.
	///
	/// Wraps [`reduced`](`reduced()`).
	pub fn reduced<'a>(
		select_fn_pin: impl 'a + Send + FnMut() -> T,
		reduce_fn_pin: impl 'a + Send + FnMut(&mut T, T) -> Propagation,
	) -> SignalArc<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a + Default,
	{
		Self::reduced_with_runtime(select_fn_pin, reduce_fn_pin, SR::default())
	}

	/// `select_fn_pin` computes each value, `reduce_fn_pin` updates current with next and can choose to [`Halt`](`Update::Halt`) propagation.
	/// Dependencies are detected across both closures.
	///
	/// Wraps [`reduced`](`reduced()`).
	pub fn reduced_with_runtime<'a>(
		select_fn_pin: impl 'a + Send + FnMut() -> T,
		reduce_fn_pin: impl 'a + Send + FnMut(&mut T, T) -> Propagation,
		runtime: SR,
	) -> SignalArc<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a,
	{
		SignalArc::new(reduced(select_fn_pin, reduce_fn_pin, runtime))
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
