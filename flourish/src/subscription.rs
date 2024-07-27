use std::{
	fmt::{self, Debug, Formatter},
	marker::PhantomData,
	mem::ManuallyDrop,
	pin::Pin,
	sync::{Arc, Weak},
};

use isoprenoid::runtime::{GlobalSignalsRuntime, Propagation, SignalsRuntimeRef};

use crate::{
	opaque::Opaque,
	traits::{Guard, Subscribable},
	unmanaged::{computed, folded, reduced},
	ArcSignal, Signal, SourcePin,
};

//TODO
// /// Type inference helper alias for [`ArcSubscription`] (using [`GlobalSignalsRuntime`]).
// pub type Subscription<T, S> = ArcSubscription<T, S, GlobalSignalsRuntime>;

/// Type of [`ArcSubscription`]s after type-erasure. Dynamic dispatch.
pub type SubscriptionDyn<'a, T, SR> = ArcSubscription<T, dyn 'a + Subscribable<T, SR>, SR>;

pub type WeakSubscriptionDyn<'a, T, SR> = WeakSubscription<T, dyn 'a + Subscribable<T, SR>, SR>;

pub struct WeakSubscription<
	T: ?Sized + Send,
	S: ?Sized + Subscribable<T, SR>,
	SR: ?Sized + SignalsRuntimeRef,
> {
	weak: Weak<Signal<T, S, SR>>,
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	WeakSubscription<T, S, SR>
{
	#[must_use]
	pub fn upgrade(&self) -> Option<ArcSignal<T, S, SR>> {
		self.signal.upgrade().map(|strong| ArcSignal {
			source: unsafe { Pin::new_unchecked(strong) },
			_phantom: PhantomData,
		})
	}
}

/// Intrinsically-subscribed version of [`ArcSignal`].  
/// Can be directly constructed but also converted to and from that type.
#[must_use = "Subscriptions are cancelled when dropped."]
pub struct ArcSubscription<
	T: ?Sized + Send,
	S: ?Sized + Subscribable<T, SR>,
	SR: ?Sized + SignalsRuntimeRef,
> {
	pub(crate) arc: Arc<Signal<T, S, SR>>,
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Debug
	for ArcSubscription<T, S, SR>
where
	T: Debug,
{
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		self.arc.clone_runtime_ref().run_detached(|| {
			f.debug_struct("ArcSubscription")
				.field("(value)", &&**self.arc.as_ref().read_exclusive_dyn())
				.finish_non_exhaustive()
		})
	}
}

unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Send
	for ArcSubscription<T, S, SR>
{
}
unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Sync
	for ArcSubscription<T, S, SR>
{
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Drop
	for ArcSubscription<T, S, SR>
{
	fn drop(&mut self) {
		self.arc.as_ref().unsubscribe();
	}
}

impl<
		'a,
		T: 'a + ?Sized + Send,
		S: 'a + Sized + Subscribable<T, SR>,
		SR: 'a + ?Sized + SignalsRuntimeRef,
	> From<ArcSubscription<T, S, SR>> for SubscriptionDyn<'a, T, SR>
{
	fn from(value: ArcSubscription<T, S, SR>) -> Self {
		value.into_dyn()
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: SignalsRuntimeRef>
	ArcSubscription<T, S, SR>
{
	/// Constructs a new [`ArcSubscription`] from the given "raw" [`Subscribable`].
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
			let arc = Arc::pin(source);
			arc.as_ref().subscribe();
			Self {
				arc,
				_phantom: PhantomData,
			}
		})
	}

	/// Cheaply borrows this [`ArcSubscription`] as [`Signal`], which is [`Copy`].
	pub fn as_ref(&self) -> Signal<'_, T, S, SR> {
		Signal {
			source: {
				let ptr =
					Arc::into_raw(unsafe { Pin::into_inner_unchecked(Pin::clone(&self.arc)) });
				unsafe { Arc::decrement_strong_count(ptr) };
				ptr
			},
			_phantom: PhantomData,
		}
	}

	/// Unsubscribes the [`ArcSubscription`], turning it into a [`ArcSignal`] in the process.
	///
	/// The underlying [`Source`](`crate::raw::Source`) may remain effectively subscribed due to subscribed dependencies.
	#[must_use = "Use `drop(self)` instead of converting first. The effect is the same."]
	pub fn unsubscribe(self) -> ArcSignal<T, S, SR> {
		//FIXME: This could avoid refcounting up and down and the associated memory barriers.
		ArcSignal {
			source: Pin::clone(&self.arc),
			_phantom: PhantomData,
		}
	} // Implicit drop(self) unsubscribes.

	/// Cheaply clones this handle into a [`ArcSignal`].
	pub fn to_signal(self) -> ArcSignal<T, S, SR> {
		ArcSignal {
			source: Pin::clone(&self.arc),
			_phantom: PhantomData,
		}
	}
}

impl<T: ?Sized + Send, S: Sized + Subscribable<T, SR>, SR: SignalsRuntimeRef>
	ArcSubscription<T, S, SR>
{
	pub fn into_dyn<'a>(self) -> SubscriptionDyn<'a, T, SR>
	where
		T: 'a,
		S: 'a,
		SR: 'a,
	{
		let this = ManuallyDrop::new(self);

		let dyn_ = SubscriptionDyn {
			arc: Pin::clone(&this.arc) as Pin<Arc<_>>,
			_phantom: PhantomData,
		};

		unsafe {
			let ptr = Arc::into_raw(Pin::into_inner_unchecked(Pin::clone(&this.arc)));
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
/// You can still easily construct them as [`ArcSignal`] and subscribe afterwards:
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
impl<T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef> ArcSubscription<T, Opaque, SR> {
	/// A simple cached computation.
	///
	/// Wraps [`computed`](`computed()`).
	pub fn computed<'a>(
		fn_pin: impl 'a + Send + FnMut() -> T,
	) -> ArcSubscription<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a + Default,
	{
		ArcSubscription::new(computed(fn_pin, SR::default()))
	}

	/// A simple cached computation.
	///
	/// Wraps [`computed`](`computed()`).
	pub fn computed_with_runtime<'a>(
		fn_pin: impl 'a + Send + FnMut() -> T,
		runtime: SR,
	) -> ArcSubscription<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a,
	{
		ArcSubscription::new(computed(fn_pin, runtime))
	}

	/// The closure mutates the value and can choose to [`Halt`](`Update::Halt`) propagation.
	///
	/// Wraps [`folded`](`folded()`).
	pub fn folded<'a>(
		init: T,
		fold_fn_pin: impl 'a + Send + FnMut(&mut T) -> Propagation,
	) -> ArcSubscription<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a + Default,
	{
		ArcSubscription::new(folded(init, fold_fn_pin, SR::default()))
	}

	/// The closure mutates the value and can choose to [`Halt`](`Update::Halt`) propagation.
	///
	/// Wraps [`folded`](`folded()`).
	pub fn folded_with_runtime<'a>(
		init: T,
		fold_fn_pin: impl 'a + Send + FnMut(&mut T) -> Propagation,
		runtime: SR,
	) -> ArcSubscription<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a,
	{
		ArcSubscription::new(folded(init, fold_fn_pin, runtime))
	}

	/// `select_fn_pin` computes each value, `reduce_fn_pin` updates current with next and can choose to [`Halt`](`Update::Halt`) propagation.
	/// Dependencies are detected across both closures.
	///
	/// Wraps [`reduced`](`reduced()`).
	pub fn reduced<'a>(
		select_fn_pin: impl 'a + Send + FnMut() -> T,
		reduce_fn_pin: impl 'a + Send + FnMut(&mut T, T) -> Propagation,
	) -> ArcSubscription<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a + Default,
	{
		ArcSubscription::new(reduced(select_fn_pin, reduce_fn_pin, SR::default()))
	}

	/// `select_fn_pin` computes each value, `reduce_fn_pin` updates current with next and can choose to [`Halt`](`Update::Halt`) propagation.
	/// Dependencies are detected across both closures.
	///
	/// Wraps [`reduced`](`reduced()`).
	pub fn reduced_with_runtime<'a>(
		select_fn_pin: impl 'a + Send + FnMut() -> T,
		reduce_fn_pin: impl 'a + Send + FnMut(&mut T, T) -> Propagation,
		runtime: SR,
	) -> ArcSubscription<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a,
	{
		ArcSubscription::new(reduced(select_fn_pin, reduce_fn_pin, runtime))
	}
}

impl<T: ?Sized + Send, S: Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	SourcePin<T, SR> for ArcSubscription<T, S, SR>
{
	fn touch(&self) {
		self.arc.as_ref().touch()
	}

	fn get_clone(&self) -> T
	where
		T: Sync + Clone,
	{
		self.arc.as_ref().get_clone()
	}

	fn get_clone_exclusive(&self) -> T
	where
		T: Clone,
	{
		self.arc.as_ref().get_clone_exclusive()
	}

	fn read<'r>(&'r self) -> S::Read<'r>
	where
		Self: Sized,
		T: 'r + Sync,
	{
		self.arc.as_ref().read()
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
		self.arc.as_ref().read_exclusive()
	}

	type ReadExclusive<'r> = S::ReadExclusive<'r>
	where
		Self: 'r + Sized,
		T: 'r;

	fn read_dyn<'r>(&'r self) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r + Sync,
	{
		self.arc.as_ref().read_dyn()
	}

	fn read_exclusive_dyn<'r>(&'r self) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r,
	{
		self.arc.as_ref().read_exclusive_dyn()
	}

	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		self.arc.as_ref().clone_runtime_ref()
	}
}

/// ⚠️ This implementation uses dynamic dispatch internally for all methods with `Self: Sized`
/// bound, which is a bit less performant than using those accessors without type erasure.
impl<'a, T: 'a + ?Sized + Send, SR: 'a + ?Sized + SignalsRuntimeRef> SourcePin<T, SR>
	for SubscriptionDyn<'a, T, SR>
{
	fn touch(&self) {
		self.arc.as_ref().touch()
	}

	fn get_clone(&self) -> T
	where
		T: Sync + Clone,
	{
		self.arc.as_ref().get_clone()
	}

	fn get_clone_exclusive(&self) -> T
	where
		T: Clone,
	{
		self.arc.as_ref().get_clone_exclusive()
	}

	fn read<'r>(&'r self) -> private::BoxedGuardDyn<'r, T>
	where
		Self: Sized,
		T: 'r + Sync,
	{
		private::BoxedGuardDyn(self.arc.as_ref().read_dyn())
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
		private::BoxedGuardDyn(self.arc.as_ref().read_exclusive_dyn())
	}

	type ReadExclusive<'r> = private::BoxedGuardDyn<'r, T>
	where
		Self: 'r + Sized,
		T: 'r;

	fn read_dyn<'r>(&'r self) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r + Sync,
	{
		self.arc.as_ref().read_dyn()
	}

	fn read_exclusive_dyn<'r>(&'r self) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r,
	{
		self.arc.as_ref().read_exclusive_dyn()
	}

	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		self.arc.as_ref().clone_runtime_ref()
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
