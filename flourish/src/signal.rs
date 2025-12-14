use std::{
	borrow::Borrow,
	cell::UnsafeCell,
	fmt::{self, Debug, Formatter},
	future::Future,
	marker::{PhantomData, PhantomPinned},
	mem::{self, ManuallyDrop, MaybeUninit},
	ops::Deref,
	pin::Pin,
	process::abort,
	sync::atomic::{AtomicUsize, Ordering},
	usize,
};

use futures_lite::FutureExt as _;
use isoprenoid::runtime::{CallbackTableTypes, Propagation, SignalsRuntimeRef};
use tap::Conv;

use crate::{
	opaque::Opaque,
	signal_arc::SignalWeakDynCell,
	traits::{UnmanagedSignal, UnmanagedSignalCell},
	unmanaged::{
		computed, computed_uncached, computed_uncached_mut, distinct, folded, reduced, InertCell,
		ReactiveCell, ReactiveCellMut, Shared,
	},
	Guard, SignalArc, SignalArcDyn, SignalArcDynCell, SignalWeak, Subscription,
};

/// A reference-counted signal.
///
/// Instances of this type can only be accessed by reference in user code.
///
/// The matching handles are [`SignalArc`], [`SignalWeak`] and [`Subscription`]:
///
/// - [`SignalArc`] and [`Subscription`] each implement both [`Borrow<Signal<…>>`](`Borrow`) and [`Deref`].
/// - [`Signal`] implements [`ToOwned<Owned = SignalArc<…>>`](`ToOwned`).
pub struct Signal<T: ?Sized + Send, S: ?Sized + Send + Sync, SR: ?Sized + SignalsRuntimeRef> {
	inner: UnsafeCell<Signal_<T, S, SR>>,
}

/// [`Signal`] after type-erasure.
pub type SignalDyn<'a, T, SR> = Signal<T, dyn 'a + UnmanagedSignal<T, SR>, SR>;
/// [`Signal`] after cell-type-erasure.
pub type SignalDynCell<'a, T, SR> = Signal<T, dyn 'a + UnmanagedSignalCell<T, SR>, SR>;

impl<T: ?Sized + Send, S: ?Sized + Send + Sync, SR: ?Sized + SignalsRuntimeRef> Signal<T, S, SR> {
	fn inner(&self) -> &Signal_<T, S, SR> {
		unsafe { &*self.inner.get().cast_const() }
	}
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef> Debug
	for Signal<T, S, SR>
where
	S: Debug,
{
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		f.debug_tuple("Signal").field(&&*self._managed()).finish()
	}
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Signal<T, S, SR>
{
	/// Creates a new [`SignalArc`] from the provided [`UnmanagedSignal`].
	///
	/// Convenience wrapper for [`SignalArc::new`].
	pub fn new(unmanaged: S) -> SignalArc<T, S, SR>
	where
		S: Sized,
	{
		SignalArc::new(unmanaged)
	}
}

/// Secondary constructors.
impl<T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef> Signal<T, Opaque, SR> {
	/// A simple cached computation.
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::GlobalSignalsRuntime;
	/// type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
	///
	/// # let input = Signal::cell(1);
	/// Signal::computed(|| input.get() + 1);
	/// # }
	/// ```
	///
	/// Wraps [`computed`](`computed()`).
	pub fn computed<'a>(
		fn_pin: impl 'a + Send + FnMut() -> T,
	) -> SignalArc<T, impl 'a + Sized + UnmanagedSignal<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a + Default,
	{
		Self::computed_with_runtime(fn_pin, SR::default())
	}

	/// A simple cached computation.
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::{GlobalSignalsRuntime, Signal};
	/// # let input = Signal::cell_with_runtime(1, GlobalSignalsRuntime);
	/// Signal::computed_with_runtime(|| input.get() + 1, input.clone_runtime_ref());
	/// # }
	/// ```
	///
	/// Wraps [`computed`](`computed()`).
	pub fn computed_with_runtime<'a>(
		fn_pin: impl 'a + Send + FnMut() -> T,
		runtime: SR,
	) -> SignalArc<T, impl 'a + Sized + UnmanagedSignal<T, SR>, SR>
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
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::GlobalSignalsRuntime;
	/// type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
	///
	/// # let input = Signal::cell(1);
	/// Signal::distinct(|| input.get() + 1);
	/// # }
	/// ```
	///
	/// Note that iff there is no subscriber,
	/// this signal and its dependents will still become stale unconditionally.
	///
	/// Wraps [`distinct`](`distinct()`).
	pub fn distinct<'a>(
		fn_pin: impl 'a + Send + FnMut() -> T,
	) -> SignalArc<T, impl 'a + Sized + UnmanagedSignal<T, SR>, SR>
	where
		T: 'a + Sized + PartialEq,
		SR: 'a + Default,
	{
		Self::distinct_with_runtime(fn_pin, SR::default())
	}

	/// A simple cached computation.
	///
	/// Doesn't update its cache or propagate iff the new result is equal.
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::{GlobalSignalsRuntime, Signal};
	/// # let input = Signal::cell_with_runtime(1, GlobalSignalsRuntime);
	/// Signal::distinct_with_runtime(|| input.get() + 1, input.clone_runtime_ref());
	/// # }
	/// ```
	///
	/// Note that iff there is no subscriber,
	/// this signal and its dependents will still become stale unconditionally.
	///
	/// Wraps [`distinct`](`distinct()`).
	pub fn distinct_with_runtime<'a>(
		fn_pin: impl 'a + Send + FnMut() -> T,
		runtime: SR,
	) -> SignalArc<T, impl 'a + Sized + UnmanagedSignal<T, SR>, SR>
	where
		T: 'a + Sized + PartialEq,
		SR: 'a,
	{
		SignalArc::new(distinct(fn_pin, runtime))
	}

	/// A simple **uncached** computation.
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::GlobalSignalsRuntime;
	/// type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
	///
	/// # let input = Signal::cell(1);
	/// Signal::computed_uncached(|| input.get() + 1);
	/// # }
	/// ```
	///
	/// Wraps [`computed_uncached`](`computed_uncached()`).
	pub fn computed_uncached<'a>(
		fn_pin: impl 'a + Send + Sync + Fn() -> T,
	) -> SignalArc<T, impl 'a + Sized + UnmanagedSignal<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a + Default,
	{
		Self::computed_uncached_with_runtime(fn_pin, SR::default())
	}

	/// A simple **uncached** computation.
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::{GlobalSignalsRuntime, Signal};
	/// # let input = Signal::cell_with_runtime(1, GlobalSignalsRuntime);
	/// Signal::computed_uncached_with_runtime(|| input.get() + 1, input.clone_runtime_ref());
	/// # }
	/// ```
	///
	/// Wraps [`computed_uncached`](`computed_uncached()`).
	pub fn computed_uncached_with_runtime<'a>(
		fn_pin: impl 'a + Send + Sync + Fn() -> T,
		runtime: SR,
	) -> SignalArc<T, impl 'a + Sized + UnmanagedSignal<T, SR>, SR>
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
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::GlobalSignalsRuntime;
	/// type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
	///
	/// # let input = Signal::cell(1);
	/// let mut read_count = 0;
	/// Signal::computed_uncached_mut(move || {
	/// 	input.touch();
	/// 	read_count += 1;
	/// 	read_count
	/// });
	/// # }
	/// ```
	///
	/// Wraps [`computed_uncached_mut`](`computed_uncached_mut()`).
	pub fn computed_uncached_mut<'a>(
		fn_pin: impl 'a + Send + FnMut() -> T,
	) -> SignalArc<T, impl 'a + Sized + UnmanagedSignal<T, SR>, SR>
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
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::{GlobalSignalsRuntime, Signal};
	/// # let input = &Signal::cell_with_runtime(1, GlobalSignalsRuntime);
	/// let mut read_count = 0;
	/// Signal::computed_uncached_mut_with_runtime(move || {
	/// 	input.touch();
	/// 	read_count += 1;
	/// 	read_count
	/// }, input.clone_runtime_ref());
	/// # }
	/// ```
	///
	/// Wraps [`computed_uncached_mut`](`computed_uncached_mut()`).
	pub fn computed_uncached_mut_with_runtime<'a>(
		fn_pin: impl 'a + Send + FnMut() -> T,
		runtime: SR,
	) -> SignalArc<T, impl 'a + Sized + UnmanagedSignal<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a,
	{
		SignalArc::new(computed_uncached_mut(fn_pin, runtime))
	}

	/// The closure mutates the value and returns a [`Propagation`].
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::{GlobalSignalsRuntime, Propagation};
	/// type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
	///
	/// # #[derive(Default, Clone)] struct Container;
	/// # impl Container { fn sort(&mut self) {} }
	/// # let input = Signal::cell(Container);
	/// Signal::folded(Container::default(), move |value| {
	/// 	value.clone_from(&input.read());
	/// 	value.sort();
	/// 	Propagation::Propagate
	/// });
	/// # }
	/// ```
	///
	/// Wraps [`folded`](`folded()`).
	pub fn folded<'a>(
		init: T,
		fold_fn_pin: impl 'a + Send + FnMut(&mut T) -> Propagation,
	) -> SignalArc<T, impl 'a + Sized + UnmanagedSignal<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a + Default,
	{
		Self::folded_with_runtime(init, fold_fn_pin, SR::default())
	}

	/// The closure mutates the value and returns a [`Propagation`].
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::{GlobalSignalsRuntime, Propagation, Signal};
	/// # #[derive(Default, Clone)] struct Container;
	/// # impl Container { fn sort(&mut self) {} }
	/// # let input = Signal::cell_with_runtime(Container, GlobalSignalsRuntime);
	/// Signal::folded_with_runtime(Container::default(), |value| {
	/// 	value.clone_from(&input.read());
	/// 	value.sort();
	/// 	Propagation::Propagate
	/// }, input.clone_runtime_ref());
	/// # }
	/// ```
	///
	/// Wraps [`folded`](`folded()`).
	pub fn folded_with_runtime<'a>(
		init: T,
		fold_fn_pin: impl 'a + Send + FnMut(&mut T) -> Propagation,
		runtime: SR,
	) -> SignalArc<T, impl 'a + Sized + UnmanagedSignal<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a,
	{
		SignalArc::new(folded(init, fold_fn_pin, runtime))
	}

	/// `select_fn_pin` computes each value.
	/// `reduce_fn_pin` updates the current value with the next and returns a [`Propagation`].
	/// Dependencies are detected across both closures.
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::{GlobalSignalsRuntime, Propagation};
	/// type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
	///
	/// # let input = Signal::cell(1);
	/// let highest_settled = Signal::reduced(
	/// 	|| input.get(),
	/// 	|value, next| if next > *value {
	/// 		*value = next;
	/// 		Propagation::Propagate
	/// 	} else {
	/// 		Propagation::Halt
	/// 	},
	/// );
	/// # }
	/// ```
	///
	/// Wraps [`reduced`](`reduced()`).
	pub fn reduced<'a>(
		select_fn_pin: impl 'a + Send + FnMut() -> T,
		reduce_fn_pin: impl 'a + Send + FnMut(&mut T, T) -> Propagation,
	) -> SignalArc<T, impl 'a + Sized + UnmanagedSignal<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a + Default,
	{
		Self::reduced_with_runtime(select_fn_pin, reduce_fn_pin, SR::default())
	}

	/// `select_fn_pin` computes each value.
	/// `reduce_fn_pin` updates the current value with the next and returns a [`Propagation`].
	/// Dependencies are detected across both closures.
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::{GlobalSignalsRuntime, Propagation, Signal};
	/// # let input = Signal::cell_with_runtime(1, GlobalSignalsRuntime);
	/// let highest_settled = Signal::reduced_with_runtime(
	/// 	|| input.get(),
	/// 	|value, next| if next > *value {
	/// 		*value = next;
	/// 		Propagation::Propagate
	/// 	} else {
	/// 		Propagation::Halt
	/// 	},
	/// 	GlobalSignalsRuntime,
	/// );
	/// # }
	/// ```
	///
	/// Wraps [`reduced`](`reduced()`).
	pub fn reduced_with_runtime<'a>(
		select_fn_pin: impl 'a + Send + FnMut() -> T,
		reduce_fn_pin: impl 'a + Send + FnMut(&mut T, T) -> Propagation,
		runtime: SR,
	) -> SignalArc<T, impl 'a + Sized + UnmanagedSignal<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a,
	{
		SignalArc::new(reduced(select_fn_pin, reduce_fn_pin, runtime))
	}

	/// A lightweight thread-safe value that's signal-compatible.
	///
	/// It doesn't have a signal-identity and isn't recorded as dependency.
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::{GlobalSignalsRuntime, Propagation};
	/// type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
	/// type SignalDyn<'a, T> = flourish::SignalDyn<'a, T, GlobalSignalsRuntime>;
	///
	/// # #[derive(Default, Clone)] struct Container;
	/// # impl Container { fn sort(&mut self) {} }
	/// # let input = Signal::cell(Container);
	/// let shared = Signal::shared(0);
	///
	/// fn accepts_signal<T: Send>(signal: &SignalDyn<'_, T>) {}
	/// accepts_signal(&*shared);
	/// # }
	/// ```
	///
	/// Since 0.1.2.
	pub fn shared<'a>(value: T) -> SignalArc<T, impl 'a + Sized + UnmanagedSignal<T, SR>, SR>
	where
		T: 'a + Sized + Sync,
		SR: 'a + Default,
	{
		Self::shared_with_runtime(value, SR::default())
	}

	/// A lightweight thread-safe value that's signal-compatible.
	///
	/// It doesn't have a signal-identity and isn't recorded as dependency.
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::{GlobalSignalsRuntime, Propagation, Signal};
	/// let shared = Signal::shared_with_runtime(0, GlobalSignalsRuntime);
	///
	/// fn accepts_signal<T: Send, SR: flourish::SignalsRuntimeRef>(
	///   signal: &flourish::SignalDyn<'_, T, SR>,
	/// ) {}
	/// accepts_signal(&*shared);
	/// # }
	/// ```
	///
	/// Since 0.1.2.
	pub fn shared_with_runtime<'a>(
		value: T,
		runtime: SR,
	) -> SignalArc<T, impl 'a + Sized + UnmanagedSignal<T, SR>, SR>
	where
		T: 'a + Sized + Sync,
		SR: 'a + Default,
	{
		SignalArc {
			strong: Strong::pin(Shared::with_runtime(value, runtime)),
		}
	}
}

/// Cell constructors.
impl<T: Send, SR: SignalsRuntimeRef> Signal<T, Opaque, SR> {
	/// A thread-safe value cell that's mutable through shared references.
	///
	/// Modification of the value can cause dependent signals to update.
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::{GlobalSignalsRuntime, Propagation};
	/// type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
	///
	/// # #[derive(Default, Clone)] struct Container;
	/// # impl Container { fn sort(&mut self) {} }
	/// let cell = Signal::cell(0);
	///
	/// cell.set_if_distinct(1);
	/// cell.set(2);
	/// cell.update(|value| {
	/// 	*value += 1;
	/// 	Propagation::Propagate
	/// });
	/// # }
	/// ```
	pub fn cell<'a>(
		initial_value: T,
	) -> SignalArc<T, impl 'a + Sized + UnmanagedSignalCell<T, SR>, SR>
	where
		T: 'a,
		SR: 'a + Default,
	{
		Self::cell_with_runtime(initial_value, SR::default())
	}

	/// A thread-safe value cell that's mutable through shared references.
	///
	/// Modification of the value can cause dependent signals to update.
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::{GlobalSignalsRuntime, Propagation, Signal};
	/// let cell = Signal::cell_with_runtime(0, GlobalSignalsRuntime);
	///
	/// cell.set_if_distinct(1);
	/// cell.set(2);
	/// cell.update(|value| {
	/// 	*value += 1;
	/// 	Propagation::Propagate
	/// });
	/// # }
	/// ```
	pub fn cell_with_runtime<'a>(
		initial_value: T,
		runtime: SR,
	) -> SignalArc<T, impl 'a + Sized + UnmanagedSignalCell<T, SR>, SR>
	where
		T: 'a,
		SR: 'a + Default,
	{
		SignalArc {
			strong: Strong::pin(InertCell::with_runtime(initial_value, runtime)),
		}
	}

	/// A thread-safe value cell that may reference itself.
	///
	/// Modification of the value can cause dependent signals to update.
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::{GlobalSignalsRuntime, Propagation, SignalsRuntimeRef, SignalWeakDynCell};
	/// type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
	///
	/// # struct Resource {}
	/// # fn get_from_cache(name: &str) -> Option<Resource> { None }
	/// # fn start_loading<SR: SignalsRuntimeRef>(name: &str, target: SignalWeakDynCell<'_, Option<Resource>, SR>) {}
	/// fn load_into<SR: SignalsRuntimeRef>(
	/// 	target: &SignalWeakDynCell<'_, Option<Resource>, SR>,
	/// 	name: &str,
	/// ) -> Option<Resource> {
	/// 	if let Some(resource) = get_from_cache(name) {
	/// 		Some(resource)
	/// 	} else {
	/// 		start_loading(name, target.clone());
	/// 		None
	/// 	}
	/// }
	///
	/// let cell = Signal::cell_cyclic(|weak| load_into(weak, "resource"));
	/// # }
	/// ```
	pub fn cell_cyclic<'a>(
		make_initial_value: impl 'a + FnOnce(&SignalWeakDynCell<'a, T, SR>) -> T,
	) -> SignalArc<T, impl 'a + Sized + UnmanagedSignalCell<T, SR>, SR>
	where
		T: 'a,
		SR: 'a + Default,
	{
		Self::cell_cyclic_with_runtime(make_initial_value, SR::default())
	}

	/// A thread-safe value cell that may reference itself.
	///
	/// Modification of the value can cause dependent signals to update.
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::{GlobalSignalsRuntime, Propagation, Signal, SignalsRuntimeRef, SignalWeakDynCell};
	/// # struct Resource {}
	/// # fn get_from_cache(name: &str) -> Option<Resource> { None }
	/// # fn start_loading<SR: SignalsRuntimeRef>(name: &str, target: SignalWeakDynCell<'_, Option<Resource>, SR>) {}
	/// fn load_into<SR: SignalsRuntimeRef>(
	/// 	target: &SignalWeakDynCell<'_, Option<Resource>, SR>,
	/// 	name: &str,
	/// ) -> Option<Resource> {
	/// 	if let Some(resource) = get_from_cache(name) {
	/// 		Some(resource)
	/// 	} else {
	/// 		start_loading(name, target.clone());
	/// 		None
	/// 	}
	/// }
	///
	/// let cell = Signal::cell_cyclic_with_runtime(
	/// 	|weak| load_into(weak, "resource"),
	/// 	GlobalSignalsRuntime,
	/// );
	/// # }
	/// ```
	pub fn cell_cyclic_with_runtime<'a>(
		make_initial_value: impl 'a + FnOnce(&SignalWeakDynCell<'a, T, SR>) -> T,
		runtime: SR,
	) -> SignalArc<T, impl 'a + Sized + UnmanagedSignalCell<T, SR>, SR>
	where
		T: 'a,
		SR: 'a + Default,
	{
		SignalArc {
			strong: Strong::pin_cyclic(|weak: &Weak<T, InertCell<T, SR>, SR>| {
				InertCell::with_runtime(
					make_initial_value(&*ManuallyDrop::new(SignalWeakDynCell {
						weak: Weak { weak: weak.weak },
					})),
					runtime,
				)
			}),
		}
	}

	/// A thread-safe value cell that can observe subscription status changes.
	///
	/// Modification of the value can cause dependent signals to update.
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::{GlobalSignalsRuntime, Propagation};
	/// type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
	///
	/// let cell = Signal::cell_reactive(0, |value, status| {
	/// 		dbg!(status);
	/// 		Propagation::Halt
	/// 	});
	/// # }
	/// ```
	pub fn cell_reactive<'a>(
		initial_value: T,
		on_subscribed_change_fn_pin: impl 'a
			+ Send
			+ FnMut(
				&T,
				<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
			) -> Propagation,
	) -> SignalArc<T, impl 'a + Sized + UnmanagedSignalCell<T, SR>, SR>
	where
		T: 'a,
		SR: 'a + Default,
	{
		Self::cell_reactive_with_runtime(initial_value, on_subscribed_change_fn_pin, SR::default())
	}

	/// A thread-safe value cell that can observe subscription status changes.
	///
	/// Modification of the value can cause dependent signals to update.
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::{GlobalSignalsRuntime, Propagation, Signal};
	/// let cell = Signal::cell_reactive_with_runtime(0, |value, status| {
	/// 		dbg!(status);
	/// 		Propagation::Halt
	/// 	}, GlobalSignalsRuntime);
	/// # }
	/// ```
	pub fn cell_reactive_with_runtime<'a>(
		initial_value: T,
		on_subscribed_change_fn_pin: impl 'a
			+ Send
			+ FnMut(
				&T,
				<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
			) -> Propagation,
		runtime: SR,
	) -> SignalArc<T, impl 'a + Sized + UnmanagedSignalCell<T, SR>, SR>
	where
		T: 'a,
		SR: 'a + Default,
	{
		SignalArc {
			strong: Strong::pin(ReactiveCell::with_runtime(
				initial_value,
				on_subscribed_change_fn_pin,
				runtime,
			)),
		}
	}

	/// A thread-safe value cell that can observe subscription status changes and may
	/// reference itself.
	///
	/// Modification of the value can cause dependent signals to update.
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::{shadow_ref_to_owned, GlobalSignalsRuntime, Propagation};
	/// type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
	///
	/// let cell = Signal::cell_cyclic_reactive(|weak| (0, {
	/// 		shadow_ref_to_owned!(weak);
	/// 		move |value, status| {
	/// 			assert_eq!(weak.upgrade().unwrap().get(), *value);
	/// 			dbg!(status);
	/// 			Propagation::Halt
	/// 	}}));
	/// # }
	/// ```
	pub fn cell_cyclic_reactive<
		'a,
		HandlerFnPin: 'a
			+ Send
			+ FnMut(
				&T,
				<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
			) -> Propagation,
	>(
		make_initial_value_and_on_subscribed_change_fn_pin: impl FnOnce(
			&SignalWeakDynCell<'a, T, SR>,
		) -> (T, HandlerFnPin),
	) -> SignalArc<T, impl 'a + Sized + UnmanagedSignalCell<T, SR>, SR>
	where
		T: 'a,
		SR: 'a + Default,
	{
		Self::cell_cyclic_reactive_with_runtime(
			make_initial_value_and_on_subscribed_change_fn_pin,
			SR::default(),
		)
	}

	/// A thread-safe value cell that can observe subscription status changes and may
	/// reference itself.
	///
	/// Modification of the value can cause dependent signals to update.
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::{shadow_ref_to_owned, GlobalSignalsRuntime, Propagation, Signal};
	/// let cell = Signal::cell_cyclic_reactive_with_runtime(|weak| (0, {
	/// 		shadow_ref_to_owned!(weak);
	/// 		move |value, status| {
	/// 			assert_eq!(weak.upgrade().unwrap().get(), *value);
	/// 			dbg!(status);
	/// 			Propagation::Halt
	/// 	}}), GlobalSignalsRuntime);
	/// # }
	/// ```
	pub fn cell_cyclic_reactive_with_runtime<
		'a,
		HandlerFnPin: 'a
			+ Send
			+ FnMut(
				&T,
				<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
			) -> Propagation,
	>(
		make_initial_value_and_on_subscribed_change_fn_pin: impl FnOnce(
			&SignalWeakDynCell<'a, T, SR>,
		) -> (T, HandlerFnPin),
		runtime: SR,
	) -> SignalArc<T, impl 'a + Sized + UnmanagedSignalCell<T, SR>, SR>
	where
		T: 'a,
		SR: 'a + Default,
	{
		SignalArc {
			strong: Strong::pin_cyclic(|weak: &Weak<T, ReactiveCell<T, HandlerFnPin, SR>, SR>| {
				let (initial_value, on_subscribed_change_fn_pin) =
					make_initial_value_and_on_subscribed_change_fn_pin(&*ManuallyDrop::new(
						SignalWeakDynCell {
							weak: Weak { weak: weak.weak },
						},
					));
				ReactiveCell::with_runtime(initial_value, on_subscribed_change_fn_pin, runtime)
			}),
		}
	}

	/// A thread-safe value cell that can observe subscription status changes.
	///
	/// Modification of the value can cause dependent signals to update.
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::{GlobalSignalsRuntime, Propagation};
	/// # fn create_heavy_resource_arc() {}
	/// type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
	///
	/// let cell = Signal::cell_reactive_mut(None, |value, status| {
	/// 		if status {
	/// 			value.get_or_insert_with(create_heavy_resource_arc);
	/// 			Propagation::Propagate
	/// 		} else {
	/// 			*value = None;
	/// 			Propagation::FlushOut
	/// 		}
	/// 	});
	/// # }
	/// ```
	pub fn cell_reactive_mut<'a>(
		initial_value: T,
		on_subscribed_change_fn_pin: impl 'a
			+ Send
			+ FnMut(
				&mut T,
				<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
			) -> Propagation,
	) -> SignalArc<T, impl 'a + Sized + UnmanagedSignalCell<T, SR>, SR>
	where
		T: 'a,
		SR: 'a + Default,
	{
		Self::cell_reactive_mut_with_runtime(
			initial_value,
			on_subscribed_change_fn_pin,
			SR::default(),
		)
	}

	/// A thread-safe value cell that can observe subscription status changes.
	///
	/// Modification of the value can cause dependent signals to update.
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::{GlobalSignalsRuntime, Propagation, Signal};
	/// # fn create_heavy_resource_arc() {}
	/// let cell = Signal::cell_reactive_mut_with_runtime(None, |value, status| {
	/// 		if status {
	/// 			value.get_or_insert_with(create_heavy_resource_arc);
	/// 			Propagation::Propagate
	/// 		} else {
	/// 			*value = None;
	/// 			Propagation::FlushOut
	/// 		}
	/// 	}, GlobalSignalsRuntime);
	/// # }
	/// ```
	pub fn cell_reactive_mut_with_runtime<'a>(
		initial_value: T,
		on_subscribed_change_fn_pin: impl 'a
			+ Send
			+ FnMut(
				&mut T,
				<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
			) -> Propagation,
		runtime: SR,
	) -> SignalArc<T, impl 'a + Sized + UnmanagedSignalCell<T, SR>, SR>
	where
		T: 'a,
		SR: 'a + Default,
	{
		SignalArc {
			strong: Strong::pin(ReactiveCellMut::with_runtime(
				initial_value,
				on_subscribed_change_fn_pin,
				runtime,
			)),
		}
	}

	/// A thread-safe value cell that can observe subscription status changes and may
	/// reference itself.
	///
	/// Modification of the value can cause dependent signals to update.
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::{shadow_ref_to_owned, GlobalSignalsRuntime, Propagation, SignalsRuntimeRef, SignalArcDynCell};
	/// type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
	///
	/// # fn start_loading<SR: SignalsRuntimeRef>(name: &str, generation: usize, target: SignalArcDynCell<'_, (usize, Resource<()>), SR>) {}
	/// enum Resource<T> {
	/// 	Offline,
	/// 	Pending,
	/// 	Ready(T),
	/// }
	///
	/// impl<T> Resource<T> {
	/// 	fn is_offline(&self) -> bool {
	/// 		matches!(self, Self::Offline)
	/// 	}
	/// }
	///
	/// let cell = Signal::cell_cyclic_reactive_mut(|weak| ((0, Resource::Offline), {
	/// 		shadow_ref_to_owned!(weak);
	/// 		move |(generation, value), status| {
	/// 			// The order of operations doesn't matter here, as the signal value is locked exclusively here.
	/// 			if status && value.is_offline() {
	/// 				*value = Resource::Pending;
	/// 				start_loading("resource", *generation, weak.upgrade().unwrap_or_else(|| unreachable!()));
	/// 				Propagation::Propagate
	/// 			} else if !status {
	/// 				*value = Resource::Offline;
	/// 				*generation += 1;
	/// 				Propagation::FlushOut
	/// 			} else {
	/// 				Propagation::Halt
	/// 			}
	/// 		}
	/// 	}));
	/// # }
	/// ```
	pub fn cell_cyclic_reactive_mut<
		'a,
		HandlerFnPin: 'a
			+ Send
			+ FnMut(
				&mut T,
				<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
			) -> Propagation,
	>(
		make_initial_value_and_on_subscribed_change_fn_pin: impl FnOnce(
			&SignalWeakDynCell<'a, T, SR>,
		) -> (T, HandlerFnPin),
	) -> SignalArc<T, impl 'a + Sized + UnmanagedSignalCell<T, SR>, SR>
	where
		T: 'a,
		SR: 'a + Default,
	{
		Self::cell_cyclic_reactive_mut_with_runtime(
			make_initial_value_and_on_subscribed_change_fn_pin,
			SR::default(),
		)
	}

	/// A thread-safe value cell that can observe subscription status changes and may
	/// reference itself.
	///
	/// Modification of the value can cause dependent signals to update.
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::{shadow_ref_to_owned, GlobalSignalsRuntime, Propagation, Signal, SignalsRuntimeRef, SignalArcDynCell};
	/// # fn start_loading<SR: SignalsRuntimeRef>(name: &str, generation: usize, target: SignalArcDynCell<'_, (usize, Resource<()>), SR>) {}
	/// enum Resource<T> {
	/// 	Offline,
	/// 	Pending,
	/// 	Ready(T),
	/// }
	///
	/// impl<T> Resource<T> {
	/// 	fn is_offline(&self) -> bool {
	/// 		matches!(self, Self::Offline)
	/// 	}
	/// }
	///
	/// let cell = Signal::cell_cyclic_reactive_mut_with_runtime(|weak| ((0, Resource::Offline), {
	/// 		shadow_ref_to_owned!(weak);
	/// 		move |(generation, value), status| {
	/// 			// The order of operations doesn't matter here, as the signal value is locked exclusively here.
	/// 			if status && value.is_offline() {
	/// 				*value = Resource::Pending;
	/// 				start_loading("resource", *generation, weak.upgrade().unwrap_or_else(|| unreachable!()));
	/// 				Propagation::Propagate
	/// 			} else if !status {
	/// 				*value = Resource::Offline;
	/// 				*generation += 1;
	/// 				Propagation::FlushOut
	/// 			} else {
	/// 				Propagation::Halt
	/// 			}
	/// 		}
	/// 	}), GlobalSignalsRuntime);
	/// # }
	/// ```
	pub fn cell_cyclic_reactive_mut_with_runtime<
		'a,
		HandlerFnPin: 'a
			+ Send
			+ FnMut(
				&mut T,
				<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
			) -> Propagation,
	>(
		make_initial_value_and_on_subscribed_change_fn_pin: impl FnOnce(
			&SignalWeakDynCell<'a, T, SR>,
		) -> (T, HandlerFnPin),
		runtime: SR,
	) -> SignalArc<T, impl 'a + Sized + UnmanagedSignalCell<T, SR>, SR>
	where
		T: 'a,
		HandlerFnPin: 'a,
		SR: 'a + Default,
	{
		SignalArc {
			strong: Strong::pin_cyclic(
				|weak: &Weak<T, ReactiveCellMut<T, HandlerFnPin, SR>, SR>| {
					let (initial_value, on_subscribed_change_fn_pin) =
						make_initial_value_and_on_subscribed_change_fn_pin(&*ManuallyDrop::new(
							SignalWeakDynCell {
								weak: Weak { weak: weak.weak },
							},
						));
					ReactiveCellMut::with_runtime(
						initial_value,
						on_subscribed_change_fn_pin,
						runtime,
					)
				},
			),
		}
	}
}

pub(crate) struct Signal_<T: ?Sized + Send, S: ?Sized + Send + Sync, SR: ?Sized + SignalsRuntimeRef>
{
	_phantom: PhantomData<(PhantomData<T>, SR)>,
	strong: AtomicUsize,
	weak: AtomicUsize,
	managed: UnsafeCell<ManuallyDrop<S>>,
}

pub(crate) struct Strong<
	T: ?Sized + Send,
	S: ?Sized + UnmanagedSignal<T, SR>,
	SR: ?Sized + SignalsRuntimeRef,
> {
	strong: *const Signal<T, S, SR>,
}

pub(crate) struct Weak<
	T: ?Sized + Send,
	S: ?Sized + UnmanagedSignal<T, SR>,
	SR: ?Sized + SignalsRuntimeRef,
> {
	weak: *const Signal<T, S, SR>,
}

/// # Safety
///
/// [`Send`] and [`Sync`] bound on `S` are implied by [`UnmanagedSignal`].
unsafe impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Send for Signal<T, S, SR>
{
}

/// # Safety
///
/// [`Send`] and [`Sync`] bound on `S` are implied by [`UnmanagedSignal`].
unsafe impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Send for Signal_<T, S, SR>
{
}

/// # Safety
///
/// [`Send`] and [`Sync`] bound on `S` are implied by [`UnmanagedSignal`].
unsafe impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Send for Strong<T, S, SR>
{
}

/// # Safety
///
/// [`Send`] and [`Sync`] bound on `S` are implied by [`UnmanagedSignal`].
unsafe impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Send for Weak<T, S, SR>
{
}

/// # Safety
///
/// [`Send`] and [`Sync`] bound on `S` are implied by [`UnmanagedSignal`].
unsafe impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Sync for Signal<T, S, SR>
{
}

/// # Safety
///
/// [`Send`] and [`Sync`] bound on `S` are implied by [`UnmanagedSignal`].
unsafe impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Sync for Signal_<T, S, SR>
{
}

/// # Safety
///
/// [`Send`] and [`Sync`] bound on `S` are implied by [`UnmanagedSignal`].
unsafe impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Sync for Strong<T, S, SR>
{
}

/// # Safety
///
/// [`Send`] and [`Sync`] bound on `S` are implied by [`UnmanagedSignal`].
unsafe impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Sync for Weak<T, S, SR>
{
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Strong<T, S, SR>
{
	pub(crate) fn pin(managed: S) -> Self
	where
		S: Sized,
	{
		Self {
			strong: Box::into_raw(Box::new(Signal {
				inner: Signal_ {
					_phantom: PhantomData,
					strong: 1.into(),
					weak: 1.into(),
					managed: UnsafeCell::new(ManuallyDrop::new(managed)),
				}
				.into(),
			})),
		}
	}
	pub(crate) fn pin_cyclic(constructor: impl FnOnce(&Weak<T, S, SR>) -> S) -> Self
	where
		S: Sized,
	{
		let weak: *const Signal<T, MaybeUninit<S>, SR> = Box::into_raw(Box::new(Signal {
			inner: Signal_ {
				_phantom: PhantomData,
				strong: 0.into(),
				weak: 1.into(),
				managed: UnsafeCell::new(ManuallyDrop::new(MaybeUninit::<S>::uninit())),
			}
			.into(),
		}))
		.cast_const();

		let weak = unsafe {
			(&mut *(*weak).inner().managed.get())
				.write(constructor(&*ManuallyDrop::new(Weak { weak: weak.cast() })));
			weak.cast::<Signal<T, S, SR>>()
		};

		(*ManuallyDrop::new(Self { strong: weak })).clone()
	}

	pub(crate) fn _get(&self) -> &Signal<T, S, SR> {
		unsafe { &*self.strong }
	}

	pub(crate) unsafe fn unsafe_copy(&self) -> Self {
		Self {
			strong: self.strong,
		}
	}

	pub(crate) fn into_dyn<'a>(self) -> Strong<T, dyn 'a + UnmanagedSignal<T, SR>, SR>
	where
		S: 'a + Sized,
	{
		let this = ManuallyDrop::new(self);
		Strong {
			strong: this.strong,
		}
	}

	pub(crate) fn into_dyn_cell<'a>(self) -> Strong<T, dyn 'a + UnmanagedSignalCell<T, SR>, SR>
	where
		S: 'a + Sized + UnmanagedSignalCell<T, SR>,
	{
		let this = ManuallyDrop::new(self);
		Strong {
			strong: this.strong,
		}
	}
}

impl<'a, T: 'a + ?Sized + Send, SR: 'a + ?Sized + SignalsRuntimeRef>
	Strong<T, dyn 'a + UnmanagedSignalCell<T, SR>, SR>
{
	pub(crate) fn into_read_only(self) -> Strong<T, dyn 'a + UnmanagedSignal<T, SR>, SR> {
		let this = ManuallyDrop::new(self);
		Strong {
			strong: this.strong,
		}
	}
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef> Deref
	for Strong<T, S, SR>
{
	type Target = Signal<T, S, SR>;

	fn deref(&self) -> &Self::Target {
		self._get()
	}
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Borrow<Signal<T, S, SR>> for Strong<T, S, SR>
{
	fn borrow(&self) -> &Signal<T, S, SR> {
		self._get()
	}
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Weak<T, S, SR>
{
	fn _inner(&self) -> &Signal_<T, S, SR> {
		unsafe { &*(*self.weak).inner.get().cast_const() }
	}

	pub(crate) fn upgrade(&self) -> Option<Strong<T, S, SR>> {
		let mut strong = self._inner().strong.load(Ordering::Relaxed);
		while strong > 0 {
			match self._inner().strong.compare_exchange(
				strong,
				strong + 1,
				Ordering::Acquire,
				Ordering::Relaxed,
			) {
				Ok(_) => return Some(Strong { strong: self.weak }),
				Err(actual) => strong = actual,
			}
		}
		None
	}

	pub(crate) unsafe fn unsafe_copy(&self) -> Self {
		Self { weak: self.weak }
	}

	pub(crate) fn into_dyn<'a>(self) -> Weak<T, dyn 'a + UnmanagedSignal<T, SR>, SR>
	where
		S: 'a + Sized,
	{
		let this = ManuallyDrop::new(self);
		Weak { weak: this.weak }
	}

	pub(crate) fn into_dyn_cell<'a>(self) -> Weak<T, dyn 'a + UnmanagedSignalCell<T, SR>, SR>
	where
		S: 'a + Sized + UnmanagedSignalCell<T, SR>,
	{
		let this = ManuallyDrop::new(self);
		Weak { weak: this.weak }
	}
}

impl<'a, T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef>
	Weak<T, dyn 'a + UnmanagedSignalCell<T, SR>, SR>
{
	pub(crate) fn into_read_only(self) -> Weak<T, dyn 'a + UnmanagedSignal<T, SR>, SR> {
		let this = ManuallyDrop::new(self);
		Weak { weak: this.weak }
	}
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef> Drop
	for Strong<T, S, SR>
{
	fn drop(&mut self) {
		if self._get().inner().strong.fetch_sub(1, Ordering::Release) == 1 {
			unsafe { ManuallyDrop::drop(&mut *self._get().inner().managed.get()) }
			drop(Weak { weak: self.strong })
		}
	}
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef> Drop
	for Weak<T, S, SR>
{
	fn drop(&mut self) {
		if self._inner().weak.fetch_sub(1, Ordering::Release) == 1 {
			unsafe {
				drop(Box::from_raw(self.weak.cast_mut()));
			}
		}
	}
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef> ToOwned
	for Signal<T, S, SR>
{
	type Owned = SignalArc<T, S, SR>;

	fn to_owned(&self) -> Self::Owned {
		(*ManuallyDrop::new(SignalArc {
			strong: Strong { strong: self },
		}))
		.clone()
	}
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef> Clone
	for Strong<T, S, SR>
{
	fn clone(&self) -> Self {
		if self._get().inner().strong.fetch_add(1, Ordering::Relaxed) > usize::MAX / 2 {
			eprintln!("SignalArc overflow.");
			abort()
		}
		Self {
			strong: self.strong,
		}
	}
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef> Clone
	for Weak<T, S, SR>
{
	fn clone(&self) -> Self {
		if self._inner().weak.fetch_add(1, Ordering::Relaxed) > usize::MAX / 2 {
			eprintln!("SignalWeak overflow.");
			abort()
		}
		Self { weak: self.weak }
	}
}

/// **Most application code should consume this.** Interface for movable signal handles that have an accessible value.
impl<T: ?Sized + Send, S: ?Sized + Send + Sync, SR: ?Sized + SignalsRuntimeRef> Signal<T, S, SR> {
	pub(crate) fn _managed(&self) -> Pin<&S> {
		unsafe { Pin::new_unchecked(&*self.inner().managed.get()) }
	}
}

/// Adapters.
impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Signal<T, S, SR>
{
	/// Creates a new [`Subscription`] for this [`Signal`].
	///
	/// Where you consume an owned [`SignalArc`], prefer [`SignalArc::into_subscription`] to avoid some memory barriers.
	pub fn to_subscription(&self) -> Subscription<T, S, SR> {
		self.to_owned().into_subscription()
	}

	/// Creates a new [`SignalWeak`] for this [`Signal`].
	pub fn downgrade(&self) -> SignalWeak<T, S, SR> {
		(*ManuallyDrop::new(SignalWeak {
			weak: Weak { weak: self },
		}))
		.clone()
	}

	/// Reborrows without the [`UnmanagedSignal`] `S` in the type signature.
	pub fn as_dyn<'a>(&self) -> &SignalDyn<'a, T, SR>
	where
		S: 'a + Sized,
	{
		self
	}

	/// Reborrows without the [`UnmanagedSignalCell`] `S` in the type signature.
	pub fn as_dyn_cell<'a>(&self) -> &SignalDynCell<'a, T, SR>
	where
		S: 'a + Sized + UnmanagedSignalCell<T, SR>,
	{
		self
	}

	/// Creates a new [`SignalArcDyn`] for this [`Signal`], without the [`UnmanagedSignal`] `S` in the type signature.
	pub fn to_dyn<'a>(&self) -> SignalArcDyn<'a, T, SR>
	where
		S: 'a + Sized,
	{
		self.to_owned().into_dyn()
	}

	/// Creates a new [`SignalArcDynCell`] for this [`Signal`], without the [`UnmanagedSignalCell`] `S` in the type signature.
	pub fn to_dyn_cell<'a>(&self) -> SignalArcDynCell<'a, T, SR>
	where
		S: 'a + Sized + UnmanagedSignalCell<T, SR>,
	{
		self.to_owned().into_dyn_cell()
	}
}

impl<T: ?Sized + Send, S: UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef> Signal<T, S, SR> {
	/// Reborrows with the [`UnmanagedSignalCell`] `S` replaced by an opaque [`UnmanagedSignal`] in the type signature.
	pub fn as_read_only<'a>(&self) -> &Signal<T, impl 'a + UnmanagedSignal<T, SR>, SR>
	where
		S: 'a + UnmanagedSignalCell<T, SR>,
	{
		self
	}

	/// Creates a new [`SignalArc`] for this [`Signal`], with the [`UnmanagedSignalCell`] `S` replaced by an opaque [`UnmanagedSignal`] in the type signature.
	pub fn to_read_only<'a>(&self) -> SignalArc<T, impl 'a + UnmanagedSignal<T, SR>, SR>
	where
		S: 'a + Sized + UnmanagedSignalCell<T, SR>,
	{
		self.to_owned()
	}
}

impl<'a, T: 'a + ?Sized + Send, SR: 'a + ?Sized + SignalsRuntimeRef> SignalDynCell<'a, T, SR> {
	/// Reborrows while upcasting the reference, discarding mutation access.
	///
	/// Since 0.1.2.
	pub fn as_read_only(&self) -> &SignalDyn<'a, T, SR> {
		self
	}

	/// Creates a new [`SignalArcDyn`] for this [`SignalDynCell`], discarding mutation access.
	///
	/// Since 0.1.2.
	pub fn to_read_only(&self) -> SignalArcDyn<'a, T, SR> {
		self.as_read_only().to_owned()
	}
}

/// Value accessors.
impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Signal<T, S, SR>
{
	/// Records `self` as dependency without accessing the value.
	pub fn touch(&self) {
		self._managed().touch()
	}

	/// Records `self` as dependency and retrieves a copy of the value.
	///
	/// Prefer [`Signal::touch`] where possible.
	pub fn get(&self) -> T
	where
		T: Sync + Copy,
	{
		self._managed().get()
	}

	/// Records `self` as dependency and retrieves a clone of the value.
	///
	/// Prefer [`Signal::get`] where available.
	pub fn get_clone(&self) -> T
	where
		T: Sync + Clone,
	{
		self._managed().get_clone()
	}

	/// Records `self` as dependency and retrieves a copy of the value.
	///
	/// Prefer [`Signal::get`] where available.
	pub fn get_exclusive(&self) -> T
	where
		T: Copy,
	{
		self._managed().get_clone_exclusive()
	}

	/// Records `self` as dependency and retrieves a clone of the value.
	///
	/// Prefer [`Signal::get_clone`] where available.
	pub fn get_clone_exclusive(&self) -> T
	where
		T: Clone,
	{
		self._managed().get_clone_exclusive()
	}

	/// Records `self` as dependency and allows borrowing the value.
	pub fn read<'r>(&'r self) -> S::Read<'r>
	where
		S: Sized,
		T: 'r + Sync,
	{
		self._managed().read()
	}

	/// Records `self` as dependency and allows borrowing the value.
	///
	/// Prefer [`Signal::read`] where available.
	pub fn read_exclusive<'r>(&'r self) -> S::ReadExclusive<'r>
	where
		S: Sized,
		T: 'r,
	{
		self._managed().read_exclusive()
	}

	/// The same as [`Signal::read`], but dyn-compatible.
	///
	/// Prefer [`Signal::read`] where available.
	pub fn read_dyn<'r>(&'r self) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r + Sync,
	{
		self._managed().read_dyn()
	}

	/// The same as [`Signal::read_exclusive`], but dyn-compatible.
	///
	/// Prefer [`Signal::read_dyn`] where available.
	pub fn read_exclusive_dyn<'r>(&'r self) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r,
	{
		self._managed().read_exclusive_dyn()
	}

	/// Clones this [`Signal`]'s [`SignalsRuntimeRef`].
	pub fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		self._managed().clone_runtime_ref()
	}
}

/// [`Cell`](`core::cell::Cell`)-likes that announce changes to their values to a [`SignalsRuntimeRef`].
///
/// The "update" and "async" methods are non-dispatchable (meaning they can't be called on trait objects).
impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignalCell<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Signal<T, S, SR>
{
	/// Iff `new_value` differs from the current value, replaces it and signals dependents.
	///
	/// # Logic
	///
	/// This method **must not** block *indefinitely*.  
	/// This method **may** defer its effect.
	pub fn set_if_distinct(&self, new_value: T)
	where
		T: 'static + Sized + PartialEq,
	{
		self._managed().set_if_distinct(new_value)
	}

	/// Unconditionally replaces the current value with `new_value` and signals dependents.
	///
	/// Prefer [`.set_if_distinct(new_value)`](`Signal::set_if_distinct`) if halting propagation is acceptable.
	///
	/// # Logic
	///
	/// This method **must not** block *indefinitely*.  
	/// This method **may** defer its effect.
	pub fn set(&self, new_value: T)
	where
		T: 'static + Sized,
	{
		self._managed().set(new_value)
	}

	/// Modifies the current value using the given closure.
	///
	/// The closure decides whether to signal dependents.
	///
	/// # Logic
	///
	/// This method **must not** block *indefinitely*.  
	/// This method **may** defer its effect.
	pub fn update(&self, update: impl 'static + Send + FnOnce(&mut T) -> Propagation)
	where
		S: Sized,
		T: 'static,
	{
		self._managed().update(update)
	}

	/// The same as [`update`](`Signal::update`), but dyn-compatible.
	pub fn update_dyn(&self, update: Box<dyn 'static + Send + FnOnce(&mut T) -> Propagation>)
	where
		T: 'static,
	{
		self._managed().update_dyn(update)
	}

	/// Cheaply creates a [`Future`] that has the effect of [`set_if_distinct_eager`](`Signal::set_if_distinct_eager`) when polled.
	/// The [`Future`] *does not* hold a strong reference to the [`Signal`].
	pub fn set_if_distinct_async<'f>(
		&self,
		new_value: T,
	) -> private::DetachedFuture<'f, Result<Result<(), T>, T>>
	where
		T: 'f + Sized + PartialEq,
		S: 'f + Sized,
		SR: 'f,
	{
		let this = self.downgrade();
		private::DetachedFuture(
			Box::pin(async move {
				if let Some(this) = this.upgrade() {
					//FIXME: Likely <https://github.com/rust-lang/rust/issues/100013>.
					this.set_if_distinct_eager(new_value).boxed().await
				} else {
					Err(new_value)
				}
			}),
			PhantomPinned,
		)
	}

	/// Cheaply creates a [`Future`] that has the effect of [`replace_if_distinct_eager`](`Signal::replace_if_distinct_eager`) when polled.
	/// The [`Future`] *does not* hold a strong reference to the [`Signal`].
	pub fn replace_if_distinct_async<'f>(
		&self,
		new_value: T,
	) -> private::DetachedFuture<'f, Result<Result<T, T>, T>>
	where
		T: 'f + Sized + PartialEq,
		S: 'f + Sized,
		SR: 'f,
	{
		let this = self.downgrade();
		private::DetachedFuture(
			Box::pin(async move {
				if let Some(this) = this.upgrade() {
					//FIXME: Likely <https://github.com/rust-lang/rust/issues/100013>.
					this.replace_if_distinct_eager(new_value).boxed().await
				} else {
					Err(new_value)
				}
			}),
			PhantomPinned,
		)
	}

	/// Cheaply creates a [`Future`] that has the effect of [`set_eager`](`Signal::set_eager`) when polled.
	/// The [`Future`] *does not* hold a strong reference to the [`Signal`].
	pub fn set_async<'f>(&self, new_value: T) -> private::DetachedFuture<'f, Result<(), T>>
	where
		T: 'f + Sized,
		S: 'f + Sized,
		SR: 'f,
	{
		let this = self.downgrade();
		private::DetachedFuture(
			Box::pin(async move {
				if let Some(this) = this.upgrade() {
					//FIXME: Likely <https://github.com/rust-lang/rust/issues/100013>.
					this.set_eager(new_value).boxed().await
				} else {
					Err(new_value)
				}
			}),
			PhantomPinned,
		)
	}

	/// Cheaply creates a [`Future`] that has the effect of [`replace_eager`](`Signal::replace_eager`) when polled.
	/// The [`Future`] *does not* hold a strong reference to the [`Signal`].
	pub fn replace_async<'f>(&self, new_value: T) -> private::DetachedFuture<'f, Result<T, T>>
	where
		T: 'f + Sized,
		S: 'f + Sized,
		SR: 'f,
	{
		let this = self.downgrade();
		private::DetachedFuture(
			Box::pin(async move {
				if let Some(this) = this.upgrade() {
					//FIXME: Likely <https://github.com/rust-lang/rust/issues/100013>.
					this.replace_eager(new_value).boxed().await
				} else {
					Err(new_value)
				}
			}),
			PhantomPinned,
		)
	}

	/// Cheaply creates a [`Future`] that has the effect of [`update_eager`](`Signal::update_eager`) when polled.
	/// The [`Future`] *does not* hold a strong reference to the [`Signal`].
	pub fn update_async<'f, U: 'f + Send, F: 'f + Send + FnOnce(&mut T) -> (Propagation, U)>(
		&self,
		update: F,
	) -> private::DetachedFuture<'f, Result<U, F>>
	where
		T: 'f,
		S: 'f + Sized,
		SR: 'f,
	{
		let this = self.downgrade();
		private::DetachedFuture(
			Box::pin(async move {
				if let Some(this) = this.upgrade() {
					//FIXME: Likely <https://github.com/rust-lang/rust/issues/100013>.
					this.update_eager(update).boxed().await
				} else {
					Err(update)
				}
			}),
			PhantomPinned,
		)
	}

	/// Cheaply creates a [`Future`] that has the effect of [`set_if_distinct_eager`](`Signal::set_if_distinct_eager`) when polled.
	/// The [`Future`] *does not* hold a strong reference to the [`Signal`].
	///
	/// Prefer [`set_if_distinct_async`](`Signal::set_if_distinct_async`) where possible.
	pub fn set_if_distinct_async_dyn<'f>(
		&self,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<Result<(), T>, T>>>
	where
		T: 'f + Sized + PartialEq,
	{
		let this = self.downgrade();
		let f = Box::new(async move {
			if let Some(this) = this.upgrade() {
				//FIXME: Likely <https://github.com/rust-lang/rust/issues/100013>.
				this.set_if_distinct_eager_dyn(new_value)
					.conv::<Pin<Box<_>>>()
					.await
			} else {
				Err(new_value)
			}
		});

		unsafe {
			//SAFETY: Lifetime extension. The closure cannot be called after `*self.source_cell`
			//        is dropped, because dropping the `RawSignal` implicitly purges the ID.
			mem::transmute::<
				Box<dyn '_ + Send + Future<Output = Result<Result<(), T>, T>>>,
				Box<dyn 'f + Send + Future<Output = Result<Result<(), T>, T>>>,
			>(f)
		}
	}

	/// Cheaply creates a [`Future`] that has the effect of [`replace_if_distinct_eager`](`Signal::replace_if_distinct_eager`) when polled.
	/// The [`Future`] *does not* hold a strong reference to the [`Signal`].
	///
	/// Prefer [`replace_if_distinct_async`](`Signal::replace_if_distinct_async`) where possible.
	pub fn replace_if_distinct_async_dyn<'f>(
		&self,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<Result<T, T>, T>>>
	where
		T: 'f + Sized + PartialEq,
	{
		let this = self.downgrade();
		let f = Box::new(async move {
			if let Some(this) = this.upgrade() {
				//FIXME: Likely <https://github.com/rust-lang/rust/issues/100013>.
				this.replace_if_distinct_eager_dyn(new_value)
					.conv::<Pin<Box<_>>>()
					.await
			} else {
				Err(new_value)
			}
		});

		unsafe {
			//SAFETY: Lifetime extension. The closure cannot be called after `*self.source_cell`
			//        is dropped, because dropping the `RawSignal` implicitly purges the ID.
			mem::transmute::<
				Box<dyn '_ + Send + Future<Output = Result<Result<T, T>, T>>>,
				Box<dyn 'f + Send + Future<Output = Result<Result<T, T>, T>>>,
			>(f)
		}
	}

	/// Cheaply creates a [`Future`] that has the effect of [`set_eager`](`Signal::set_eager`) when polled.
	/// The [`Future`] *does not* hold a strong reference to the [`Signal`].
	///
	/// Prefer [`set_async`](`Signal::set_async`) where possible.
	pub fn set_async_dyn<'f>(
		&self,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<(), T>>>
	where
		T: 'f + Sized,
	{
		let this = self.downgrade();
		let f = Box::new(async move {
			if let Some(this) = this.upgrade() {
				//FIXME: Likely <https://github.com/rust-lang/rust/issues/100013>.
				this.set_eager_dyn(new_value).conv::<Pin<Box<_>>>().await
			} else {
				Err(new_value)
			}
		});

		unsafe {
			//SAFETY: Lifetime extension. The closure cannot be called after `*self.source_cell`
			//        is dropped, because dropping the `RawSignal` implicitly purges the ID.
			mem::transmute::<
				Box<dyn '_ + Send + Future<Output = Result<(), T>>>,
				Box<dyn 'f + Send + Future<Output = Result<(), T>>>,
			>(f)
		}
	}

	/// Cheaply creates a [`Future`] that has the effect of [`replace_eager`](`Signal::replace_eager`) when polled.
	/// The [`Future`] *does not* hold a strong reference to the [`Signal`].
	///
	/// Prefer [`replace_async`](`Signal::replace_async`) where possible.
	pub fn replace_async_dyn<'f>(
		&self,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<T, T>>>
	where
		T: 'f + Sized,
	{
		let this = self.downgrade();
		let f = Box::new(async move {
			if let Some(this) = this.upgrade() {
				//FIXME: Likely <https://github.com/rust-lang/rust/issues/100013>.
				this.replace_eager_dyn(new_value)
					.conv::<Pin<Box<_>>>()
					.await
			} else {
				Err(new_value)
			}
		});

		unsafe {
			//SAFETY: Lifetime extension. The closure cannot be called after `*self.source_cell`
			//        is dropped, because dropping the `RawSignal` implicitly purges the ID.
			mem::transmute::<
				Box<dyn '_ + Send + Future<Output = Result<T, T>>>,
				Box<dyn 'f + Send + Future<Output = Result<T, T>>>,
			>(f)
		}
	}

	/// Cheaply creates a [`Future`] that has the effect of [`update_eager`](`Signal::update_eager`) when polled.
	/// The [`Future`] *does not* hold a strong reference to the [`Signal`].
	///
	/// Prefer [`update_async`](`Signal::update_async`) where possible.
	pub fn update_async_dyn<'f>(
		&self,
		update: Box<dyn 'f + Send + FnOnce(&mut T) -> Propagation>,
	) -> Box<
		dyn 'f
			+ Send
			+ Future<Output = Result<(), Box<dyn 'f + Send + FnOnce(&mut T) -> Propagation>>>,
	>
	where
		T: 'f,
	{
		let this = self.downgrade();
		let f = Box::new(async move {
			if let Some(this) = this.upgrade() {
				let f: Pin<Box<_>> = this.update_eager_dyn(update).into();
				f.await
			} else {
				Err(update)
			}
		});

		unsafe {
			//SAFETY: Lifetime extension. The closure cannot be called after `*self.source_cell`
			//        is dropped, because dropping the `RawSignal` implicitly purges the ID.
			mem::transmute::<
				Box<
					dyn '_
						+ Send
						+ Future<
							Output = Result<(), Box<dyn 'f + Send + FnOnce(&mut T) -> Propagation>>,
						>,
				>,
				Box<
					dyn 'f
						+ Send
						+ Future<
							Output = Result<(), Box<dyn 'f + Send + FnOnce(&mut T) -> Propagation>>,
						>,
				>,
			>(f)
		}
	}

	/// Iff `new_value` differs from the current value, overwrites it and signals dependents.
	///
	/// # Returns
	///
	/// [`Ok`], or [`Err(new_value)`](`Err`) iff not replaced.
	///
	/// # Panics
	///
	/// The returned [`Future`] **may** panic if polled in signal callbacks.
	///
	/// # Logic
	///
	/// This method **must not** block *indefinitely*.  
	/// This method **should** schedule its effect even if the returned [`Future`] is not polled.  
	/// This method **should** cancel its effect when the returned [`Future`] is dropped.  
	/// The returned [`Future`] **may** return [`Pending`](`core::task::Poll::Pending`) indefinitely iff polled in signal callbacks.
	///
	/// Don't `.await` the returned [`Future`] in signal callbacks!
	pub fn set_if_distinct_eager<'f>(&self, new_value: T) -> S::SetIfDistinctEager<'f>
	where
		S: 'f + Sized,
		T: 'f + Sized + PartialEq,
	{
		self._managed().set_if_distinct_eager(new_value)
	}

	/// Iff `new_value` differs from the current value, replaces it and signals dependents.
	///
	/// # Returns
	///
	/// [`Ok`] with the previous value, or [`Err(new_value)`](`Err`) iff not replaced.
	///
	/// # Panics
	///
	/// The returned [`Future`] **may** panic if polled in signal callbacks.
	///
	/// # Logic
	///
	/// This method **must not** block *indefinitely*.  
	/// This method **should** schedule its effect even if the returned [`Future`] is not polled.  
	/// This method **should** cancel its effect when the returned [`Future`] is dropped.  
	/// The returned [`Future`] **may** return [`Pending`](`core::task::Poll::Pending`) indefinitely iff polled in signal callbacks.
	///
	/// Don't `.await` the returned [`Future`] in signal callbacks!
	pub fn replace_if_distinct_eager<'f>(&self, new_value: T) -> S::ReplaceIfDistinctEager<'f>
	where
		S: 'f + Sized,
		T: 'f + Sized + PartialEq,
	{
		self._managed().replace_if_distinct_eager(new_value)
	}

	/// Unconditionally overwrites the current value with `new_value` and signals dependents.
	///
	/// # Returns
	///
	/// The previous value.
	///
	/// # Panics
	///
	/// The returned [`Future`] **may** panic if polled in signal callbacks.
	///
	/// # Logic
	///
	/// This method **must not** block *indefinitely*.  
	/// This method **should** schedule its effect even if the returned [`Future`] is not polled.  
	/// This method **should** cancel its effect when the returned [`Future`] is dropped.  
	/// The returned [`Future`] **may** return [`Pending`](`core::task::Poll::Pending`) indefinitely iff polled in signal callbacks.
	///
	/// Don't `.await` the returned [`Future`] in signal callbacks!
	pub fn set_eager<'f>(&self, new_value: T) -> S::SetEager<'f>
	where
		S: 'f + Sized,
		T: 'f + Sized,
	{
		self._managed().set_eager(new_value)
	}

	/// Unconditionally replaces the current value with `new_value` and signals dependents.
	///
	/// # Returns
	///
	/// The previous value.
	///
	/// # Panics
	///
	/// The returned [`Future`] **may** panic if polled in signal callbacks.
	///
	/// # Logic
	///
	/// This method **must not** block *indefinitely*.  
	/// This method **should** schedule its effect even if the returned [`Future`] is not polled.  
	/// This method **should** cancel its effect when the returned [`Future`] is dropped.  
	/// The returned [`Future`] **may** return [`Pending`](`core::task::Poll::Pending`) indefinitely iff polled in signal callbacks.
	///
	/// Don't `.await` the returned [`Future`] in signal callbacks!
	pub fn replace_eager<'f>(&self, new_value: T) -> S::ReplaceEager<'f>
	where
		S: 'f + Sized,
		T: 'f + Sized,
	{
		self._managed().replace_eager(new_value)
	}

	/// Modifies the current value using the given closure.
	///
	/// The closure decides whether to signal dependents.
	///
	/// # Returns
	///
	/// The `U` returned by `update`.
	///
	/// # Panics
	///
	/// The returned [`Future`] **may** panic if polled in signal callbacks.
	///
	/// # Logic
	///
	/// This method **must not** block *indefinitely*.  
	/// This method **should** schedule its effect even if the returned [`Future`] is not polled.  
	/// This method **should** cancel its effect when the returned [`Future`] is dropped.  
	/// The returned [`Future`] **may** return [`Pending`](`core::task::Poll::Pending`) indefinitely iff polled in signal callbacks.
	///
	/// Don't `.await` the returned [`Future`] in signal callbacks!
	pub fn update_eager<'f, U: Send, F: 'f + Send + FnOnce(&mut T) -> (Propagation, U)>(
		&self,
		update: F,
	) -> S::UpdateEager<'f, U, F>
	where
		S: 'f + Sized,
	{
		self._managed().update_eager(update)
	}

	/// The same as [`set_if_distinct_eager`](`Signal::set_if_distinct_eager`), but dyn-compatible.
	pub fn set_if_distinct_eager_dyn<'f>(
		&self,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<Result<(), T>, T>>>
	where
		T: 'f + Sized + PartialEq,
	{
		self._managed().set_if_distinct_eager_dyn(new_value)
	}

	/// The same as [`replace_if_distinct_eager`](`Signal::replace_if_distinct_eager`), but dyn-compatible.
	pub fn replace_if_distinct_eager_dyn<'f>(
		&self,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<Result<T, T>, T>>>
	where
		T: 'f + Sized + PartialEq,
	{
		self._managed().replace_if_distinct_eager_dyn(new_value)
	}

	/// The same as [`set_eager`](`Signal::set_eager`), but dyn-compatible.
	pub fn set_eager_dyn<'f>(
		&self,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<(), T>>>
	where
		T: 'f + Sized,
	{
		self._managed().set_eager_dyn(new_value)
	}

	/// The same as [`replace_eager`](`Signal::replace_eager`), but dyn-compatible.
	pub fn replace_eager_dyn<'f>(
		&self,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<T, T>>>
	where
		T: 'f + Sized,
	{
		self._managed().replace_eager_dyn(new_value)
	}

	/// The same as [`update_eager`](`Signal::update_eager`), but dyn-compatible.
	pub fn update_eager_dyn<'f>(
		&self,
		update: Box<dyn 'f + Send + FnOnce(&mut T) -> Propagation>,
	) -> Box<
		dyn 'f
			+ Send
			+ Future<Output = Result<(), Box<dyn 'f + Send + FnOnce(&mut T) -> Propagation>>>,
	>
	where
		T: 'f,
	{
		self._managed().update_eager_dyn(update)
	}

	/// Iff `new_value` differs from the current value, overwrites it and signals dependents.
	///
	/// # Returns
	///
	/// [`Ok`], or [`Err(new_value)`](`Err`) iff not replaced.
	///
	/// # Panics
	///
	/// This method **may** panic if called in signal callbacks.
	///
	/// # Logic
	///
	/// This method **may** block *indefinitely* iff called in signal callbacks.
	pub fn set_if_distinct_blocking(&self, new_value: T) -> Result<(), T>
	where
		T: Sized + PartialEq,
	{
		self._managed().set_if_distinct_blocking(new_value)
	}

	/// Iff `new_value` differs from the current value, replaces it and signals dependents.
	///
	/// # Returns
	///
	/// [`Ok`] with the previous value, or [`Err(new_value)`](`Err`) iff not replaced.
	///
	/// # Panics
	///
	/// This method **may** panic if called in signal callbacks.
	///
	/// # Logic
	///
	/// This method **may** block *indefinitely* iff called in signal callbacks.
	pub fn replace_if_distinct_blocking(&self, new_value: T) -> Result<T, T>
	where
		T: Sized + PartialEq,
	{
		self._managed().replace_if_distinct_blocking(new_value)
	}

	/// Unconditionally overwrites the current value with `new_value` and signals dependents.
	///
	/// # Panics
	///
	/// This method **may** panic if called in signal callbacks.
	///
	/// # Logic
	///
	/// This method **may** block *indefinitely* iff called in signal callbacks.
	pub fn set_blocking(&self, new_value: T)
	where
		T: Sized,
	{
		self._managed().set_blocking(new_value)
	}

	/// Unconditionally replaces the current value with `new_value` and signals dependents.
	///
	/// # Returns
	///
	/// The previous value.
	///
	/// # Panics
	///
	/// This method **may** panic if called in signal callbacks.
	///
	/// # Logic
	///
	/// This method **may** block *indefinitely* iff called in signal callbacks.
	pub fn replace_blocking(&self, new_value: T) -> T
	where
		T: Sized,
	{
		self._managed().replace_blocking(new_value)
	}

	/// Modifies the current value using the given closure.
	///
	/// The closure decides whether to signal dependents.
	///
	/// # Returns
	///
	/// The `U` returned by `update`.
	///
	/// # Panics
	///
	/// This method **may** panic if called in signal callbacks.
	///
	/// # Logic
	///
	/// This method **may** block *indefinitely* iff called in signal callbacks.
	pub fn update_blocking<U>(&self, update: impl FnOnce(&mut T) -> (Propagation, U)) -> U
	where
		S: Sized,
	{
		self._managed().update_blocking(update)
	}

	/// The same as [`update_blocking`](`Signal::update_blocking`), but dyn-compatible.
	pub fn update_blocking_dyn(&self, update: Box<dyn '_ + FnOnce(&mut T) -> Propagation>) {
		self._managed().update_blocking_dyn(update)
	}
}

/// Duplicated to avoid identities.
mod private {
	use std::{
		future::Future,
		marker::PhantomPinned,
		pin::Pin,
		task::{Context, Poll},
	};

	use futures_lite::FutureExt;
	use pin_project::pin_project;

	#[must_use = "Async futures have no effect iff dropped before polling (and may cancel their effect iff dropped)."]
	#[pin_project]
	pub struct DetachedFuture<'f, Output: 'f>(
		pub(super) Pin<Box<dyn 'f + Send + Future<Output = Output>>>,
		#[pin] pub(super) PhantomPinned,
	);

	impl<'f, Output: 'f> Future for DetachedFuture<'f, Output> {
		type Output = Output;

		fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
			self.project().0.poll(cx)
		}
	}
}
