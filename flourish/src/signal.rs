use std::{
	fmt::{self, Debug, Formatter},
	marker::PhantomData,
	pin::Pin,
	sync::{Arc, Weak},
};

use isoprenoid::runtime::{GlobalSignalsRuntime, Propagation, SignalsRuntimeRef};

use crate::{
	opaque::Opaque,
	raw::{computed, computed_uncached, computed_uncached_mut, debounced, folded, reduced},
	subscription::SubscriptionDyn,
	traits::{Guard, Subscribable},
	SourcePin, SubscriptionSR,
};

/// Type inference helper alias for [`SignalSR`] (using [`GlobalSignalsRuntime`]).
pub type Signal<T, S> = SignalSR<T, S, GlobalSignalsRuntime>;

/// Type of [`SignalSR`]s after type-erasure. Dynamic dispatch.
pub type SignalDyn<'a, T, SR> = SignalSR<T, dyn 'a + Subscribable<T, SR>, SR>;

/// Type of [`WeakSignal`]s after type-erasure or [`SignalDyn`] after downgrade. Dynamic dispatch.
pub type WeakSignalDyn<'a, T, SR> = WeakSignal<T, dyn 'a + Subscribable<T, SR>, SR>;

pub struct WeakSignal<
	T: ?Sized + Send,
	S: ?Sized + Subscribable<T, SR>,
	SR: ?Sized + SignalsRuntimeRef,
> {
	source_cell: Weak<S>,
	_phantom: PhantomData<(PhantomData<T>, SR)>,
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	WeakSignal<T, S, SR>
{
	#[must_use]
	pub fn upgrade(&self) -> Option<SignalSR<T, S, SR>> {
		self.source_cell.upgrade().map(|strong| SignalSR {
			source: unsafe { Pin::new_unchecked(strong) },
			_phantom: PhantomData,
		})
	}
}

/// A largely type-erased signal handle that is all of [`Clone`], [`Send`], [`Sync`] and [`Unpin`].
///
/// To access values, import [`SourcePin`].
///
/// Signals are not evaluated unless they are subscribed-to (or on demand if if not current).  
/// Uncached signals are instead evaluated on direct demand **only** (but still communicate subscriptions and invalidation).
pub struct SignalSR<
	T: ?Sized + Send,
	S: ?Sized + Subscribable<T, SR>,
	SR: ?Sized + SignalsRuntimeRef,
> {
	pub(super) source: Pin<Arc<S>>,
	pub(crate) _phantom: PhantomData<(PhantomData<T>, SR)>,
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Clone
	for SignalSR<T, S, SR>
{
	fn clone(&self) -> Self {
		Self {
			source: self.source.clone(),
			_phantom: PhantomData,
		}
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Debug
	for SignalSR<T, S, SR>
where
	T: Debug,
{
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		self.source.clone_runtime_ref().run_detached(|| {
			f.debug_struct("SignalSR")
				.field("(value)", &&**self.source.as_ref().read_exclusive_dyn())
				.finish_non_exhaustive()
		})
	}
}

unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Send
	for SignalSR<T, S, SR>
{
}
unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Sync
	for SignalSR<T, S, SR>
{
}

impl<
		'a,
		T: 'a + ?Sized + Send,
		S: 'a + Sized + Subscribable<T, SR>,
		SR: 'a + ?Sized + SignalsRuntimeRef,
	> From<SignalSR<T, S, SR>> for SignalDyn<'a, T, SR>
{
	fn from(value: SignalSR<T, S, SR>) -> Self {
		let SignalSR { source, _phantom } = value;
		Self { source, _phantom }
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	SignalSR<T, S, SR>
{
	/// Creates a new [`SignalSR`] from the provided raw [`Subscribable`].
	pub fn new(source: S) -> Self
	where
		S: Sized,
	{
		SignalSR {
			source: Arc::pin(source),
			_phantom: PhantomData,
		}
	}

	/// Cheaply borrows this [`SignalSR`] as [`SignalRef`], which is [`Copy`].
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

	//TODO: Various `From` and `TryFrom` conversions, including for unsizing.

	pub fn try_subscribe(self) -> Result<SubscriptionSR<T, S, SR>, Self> {
		if self.source.as_ref().subscribe_inherently() {
			Ok(SubscriptionSR {
				source: self.source,
				_phantom: PhantomData,
			})
		} else {
			Err(self)
		}
	}

	pub fn into_dyn<'a>(self) -> SignalDyn<'a, T, SR>
	where
		S: 'a + Sized,
	{
		let Self { source, _phantom } = self;
		SignalDyn { source, _phantom }
	}
}

impl<T: ?Sized + Send, S: Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	SignalSR<T, S, SR>
{
	/// First calls [`self.try_subscribe()`](`SignalSR::try_subscribe`) and, iff that fails,
	/// falls back to constructing a computed (cached) subscription from `make_fn_pin(self)`'s output.
	pub fn subscribe_or_computed_dyn<'a, FnPin: 'a + Send + FnMut() -> T>(
		self,
		make_fn_pin: impl FnOnce(Self) -> FnPin,
	) -> SubscriptionDyn<'a, T, SR>
	where
		T: 'a + Sized,
		S: 'a,
		SR: 'a,
	{
		self.try_subscribe()
			.map(|subscription| subscription.into_dyn())
			.unwrap_or_else(move |this| {
				let runtime = this.clone_runtime_ref();
				SubscriptionSR::computed_with_runtime(make_fn_pin(this), runtime).into_dyn()
			})
	}
}

impl<'a, T: 'a + ?Sized + Send, SR: 'a + ?Sized + SignalsRuntimeRef> SignalDyn<'a, T, SR> {
	/// First calls [`self.try_subscribe()`](`SignalSR::try_subscribe`) and, iff that fails,
	/// falls back to constructing a computed (cached) subscription from `make_fn_pin(self)`'s output.
	pub fn subscribe_or_computed_dyn<FnPin: 'a + Send + FnMut() -> T>(
		self,
		make_fn_pin: impl FnOnce(Self) -> FnPin,
	) -> SubscriptionDyn<'a, T, SR>
	where
		T: Sized,
		SR: Sized,
	{
		self.try_subscribe()
			.map(|subscription| subscription)
			.unwrap_or_else(move |this| {
				let runtime = this.clone_runtime_ref();
				SubscriptionSR::computed_with_runtime(make_fn_pin(this), runtime).into_dyn()
			})
	}
}

/// Secondary constructors.
impl<T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef> SignalSR<T, Opaque, SR> {
	/// A simple cached computation.
	///
	/// Wraps [`computed`](`computed()`).
	pub fn computed<'a>(
		fn_pin: impl 'a + Send + FnMut() -> T,
	) -> SignalSR<T, impl 'a + Sized + Subscribable<T, SR>, SR>
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
	) -> SignalSR<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a,
	{
		SignalSR::new(computed(fn_pin, runtime))
	}

	/// A simple cached computation.
	///
	/// Doesn't update its cache or propagate iff the new result is equal.
	///
	/// Wraps [`debounced`](`debounced()`).
	pub fn debounced<'a>(
		fn_pin: impl 'a + Send + FnMut() -> T,
	) -> SignalSR<T, impl 'a + Sized + Subscribable<T, SR>, SR>
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
	) -> SignalSR<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized + PartialEq,
		SR: 'a,
	{
		SignalSR::new(debounced(fn_pin, runtime))
	}

	/// A simple **uncached** computation.
	///
	/// Wraps [`computed_uncached`](`computed_uncached()`).
	pub fn computed_uncached<'a>(
		fn_pin: impl 'a + Send + Sync + Fn() -> T,
	) -> SignalSR<T, impl 'a + Sized + Subscribable<T, SR>, SR>
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
	) -> SignalSR<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a,
	{
		SignalSR::new(computed_uncached(fn_pin, runtime))
	}

	/// A simple **stateful uncached** computation.
	///
	/// ⚠️ Care must be taken to avoid unexpected behaviour!
	///
	/// Wraps [`computed_uncached_mut`](`computed_uncached_mut()`).
	pub fn computed_uncached_mut<'a>(
		fn_pin: impl 'a + Send + FnMut() -> T,
	) -> SignalSR<T, impl 'a + Sized + Subscribable<T, SR>, SR>
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
	) -> SignalSR<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a,
	{
		SignalSR::new(computed_uncached_mut(fn_pin, runtime))
	}

	/// The closure mutates the value and can choose to [`Halt`](`Update::Halt`) propagation.
	///
	/// Wraps [`folded`](`folded()`).
	pub fn folded<'a>(
		init: T,
		fold_fn_pin: impl 'a + Send + FnMut(&mut T) -> Propagation,
	) -> SignalSR<T, impl 'a + Sized + Subscribable<T, SR>, SR>
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
	) -> SignalSR<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a,
	{
		SignalSR::new(folded(init, fold_fn_pin, runtime))
	}

	/// `select_fn_pin` computes each value, `reduce_fn_pin` updates current with next and can choose to [`Halt`](`Update::Halt`) propagation.
	/// Dependencies are detected across both closures.
	///
	/// Wraps [`reduced`](`reduced()`).
	pub fn reduced<'a>(
		select_fn_pin: impl 'a + Send + FnMut() -> T,
		reduce_fn_pin: impl 'a + Send + FnMut(&mut T, T) -> Propagation,
	) -> SignalSR<T, impl 'a + Sized + Subscribable<T, SR>, SR>
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
	) -> SignalSR<T, impl 'a + Sized + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a,
	{
		SignalSR::new(reduced(select_fn_pin, reduce_fn_pin, runtime))
	}
}

impl<T: ?Sized + Send, S: Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	SourcePin<T, SR> for SignalSR<T, S, SR>
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
		Self: 'r + Sized,
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
		Self: 'r + Sized,
		T: 'r,
	{
		self.source.as_ref().read_exclusive()
	}

	type ReadExclusive<'r> = S::ReadExclusive<'r>
	where
		Self: 'r + Sized;

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

/// ⚠️ This implementation uses dynamic dispatch internally for all methods with `Self: Sized`
/// bound, which is a bit less performant than using those accessors without type erasure.
impl<'a, T: 'a + ?Sized + Send, SR: 'a + ?Sized + SignalsRuntimeRef> SourcePin<T, SR>
	for SignalDyn<'a, T, SR>
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
		Self: 'r + Sized,
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
		Self: 'r + Sized,
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

/// Type of [`SignalRef`]s after type-erasure. Dynamic dispatch.
pub type SignalRefDyn<'r, 'a, T, SR> = SignalRef<'r, T, dyn 'a + Subscribable<T, SR>, SR>;

/// A very cheap [`SignalSR`]-like borrow that's [`Copy`].
///
/// Can be cloned into an additional [`SignalSR`] and indirectly subscribed to.
#[derive(Debug)]
pub struct SignalRef<
	'r,
	T: ?Sized + Send,
	S: ?Sized + Subscribable<T, SR>,
	SR: ?Sized + SignalsRuntimeRef,
> {
	pub(crate) source: *const S,
	pub(crate) _phantom: PhantomData<&'r (PhantomData<T>, SR)>,
}

impl<'r, T: Send + ?Sized, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	SignalRef<'r, T, S, SR>
{
	/// Cheaply creates an additional [`SignalSR`] managing the same [`Subscribable`].
	pub fn to_signal(&self) -> SignalSR<T, S, SR> {
		SignalSR {
			source: unsafe {
				Arc::increment_strong_count(self.source);
				Pin::new_unchecked(Arc::from_raw(self.source))
			},
			_phantom: PhantomData,
		}
	}

	/// Creates a computed (cached) [`SubscriptionSR`] based on this [`SignalRef`].
	///
	/// This is a shortcut past `self.to_signal().subscribe_or_computed(make_fn_pin)`.  
	/// (This method may be slightly more efficient.)
	pub fn subscribe_computed<'a, FnPin: 'a + Send + FnMut() -> T>(
		&self,
		make_fn_pin: impl FnOnce(SignalSR<T, S, SR>) -> FnPin,
	) -> SubscriptionSR<T, impl 'a + Subscribable<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a,
	{
		SubscriptionSR::computed_with_runtime(
			make_fn_pin(self.to_signal()),
			unsafe { Pin::new_unchecked(&*self.source) }.clone_runtime_ref(),
		)
	}
}

impl<'r, T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Clone
	for SignalRef<'r, T, S, SR>
{
	fn clone(&self) -> Self {
		*self
	}
}

impl<'r, T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Copy
	for SignalRef<'r, T, S, SR>
{
}

unsafe impl<'r, T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Send for SignalRef<'r, T, S, SR>
{
	// SAFETY: The [`Subscribable`] used internally requires both [`Send`] and [`Sync`] of the underlying object.
}

unsafe impl<'r, T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Sync for SignalRef<'r, T, S, SR>
{
	// SAFETY: The [`Subscribable`] used internally requires both [`Send`] and [`Sync`] of the underlying object.
}

impl<'r, T: ?Sized + Send, S: Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	SourcePin<T, SR> for SignalRef<'r, T, S, SR>
{
	//SAFETY: `self.source` is a payload pointer that's valid for at least 'r.

	fn touch(&self) {
		unsafe { Pin::new_unchecked(&*self.source) }.touch()
	}

	fn get_clone(&self) -> T
	where
		T: Sync + Clone,
	{
		unsafe { Pin::new_unchecked(&*self.source) }.get_clone()
	}

	fn get_clone_exclusive(&self) -> T
	where
		T: Clone,
	{
		unsafe { Pin::new_unchecked(&*self.source) }.get_clone_exclusive()
	}

	fn read<'r_>(&'r_ self) -> S::Read<'r_>
	where
		Self: Sized,
		T: 'r_ + Sync,
	{
		unsafe { Pin::new_unchecked(&*self.source) }.read()
	}

	type Read<'r_> = S::Read<'r_>
	where
		Self: 'r_ + Sized,
		T: 'r_ + Sync;

	fn read_exclusive<'r_>(&'r_ self) -> S::ReadExclusive<'r_>
	where
		Self: Sized,
		T: 'r_,
	{
		unsafe { Pin::new_unchecked(&*self.source) }.read_exclusive()
	}

	type ReadExclusive<'r_> = S::ReadExclusive<'r_>
	where
		Self: 'r_ + Sized,
		T: 'r_;

	fn read_dyn<'r_>(&'r_ self) -> Box<dyn 'r_ + Guard<T>>
	where
		T: 'r_ + Sync,
	{
		unsafe { Pin::new_unchecked(&*self.source) }.read_dyn()
	}

	fn read_exclusive_dyn<'r_>(&'r_ self) -> Box<dyn 'r_ + Guard<T>>
	where
		T: 'r_,
	{
		unsafe { Pin::new_unchecked(&*self.source) }.read_exclusive_dyn()
	}

	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		unsafe { Pin::new_unchecked(&*self.source) }.clone_runtime_ref()
	}

	fn get(&self) -> T
	where
		T: Sync + Copy,
	{
		unsafe { Pin::new_unchecked(&*self.source) }.get()
	}

	fn get_exclusive(&self) -> T
	where
		T: Copy,
	{
		unsafe { Pin::new_unchecked(&*self.source) }.get_exclusive()
	}
}

/// ⚠️ This implementation uses dynamic dispatch internally for all methods with `Self: Sized`
/// bound, which is a bit less performant than using those accessors without type erasure.
impl<'r, 'a, T: 'a + ?Sized + Send, SR: ?Sized + SignalsRuntimeRef> SourcePin<T, SR>
	for SignalRefDyn<'r, 'a, T, SR>
{
	//SAFETY: `self.source` is a payload pointer that's valid for at least 'r.

	fn touch(&self) {
		unsafe { Pin::new_unchecked(&*self.source) }.touch()
	}

	fn get_clone(&self) -> T
	where
		T: Sync + Clone,
	{
		unsafe { Pin::new_unchecked(&*self.source) }.get_clone()
	}

	fn get_clone_exclusive(&self) -> T
	where
		T: Clone,
	{
		unsafe { Pin::new_unchecked(&*self.source) }.get_clone_exclusive()
	}

	fn read<'r_>(&'r_ self) -> private::BoxedGuardDyn<'r_, T>
	where
		Self: Sized,
		T: 'r_ + Sync,
	{
		private::BoxedGuardDyn(unsafe { Pin::new_unchecked(&*self.source) }.read_dyn())
	}

	type Read<'r_> = private::BoxedGuardDyn<'r_, T>
	where
		Self: 'r_ + Sized,
		T: 'r_ + Sync;

	fn read_exclusive<'r_>(&'r_ self) -> private::BoxedGuardDyn<'r_, T>
	where
		Self: Sized,
		T: 'r_,
	{
		private::BoxedGuardDyn(unsafe { Pin::new_unchecked(&*self.source) }.read_exclusive_dyn())
	}

	type ReadExclusive<'r_> = private::BoxedGuardDyn<'r_, T>
	where
		Self: 'r_ + Sized,
		T: 'r_;

	fn read_dyn<'r_>(&'r_ self) -> Box<dyn 'r_ + Guard<T>>
	where
		T: 'r_ + Sync,
	{
		unsafe { Pin::new_unchecked(&*self.source) }.read_dyn()
	}

	fn read_exclusive_dyn<'r_>(&'r_ self) -> Box<dyn 'r_ + Guard<T>>
	where
		T: 'r_,
	{
		unsafe { Pin::new_unchecked(&*self.source) }.read_exclusive_dyn()
	}

	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		unsafe { Pin::new_unchecked(&*self.source) }.clone_runtime_ref()
	}

	fn get(&self) -> T
	where
		T: Sync + Copy,
	{
		unsafe { Pin::new_unchecked(&*self.source) }.get()
	}

	fn get_exclusive(&self) -> T
	where
		T: Copy,
	{
		unsafe { Pin::new_unchecked(&*self.source) }.get_exclusive()
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
