use std::{
	borrow::Borrow,
	fmt::{self, Debug, Formatter},
	future::Future,
	mem::{ManuallyDrop, MaybeUninit},
	ops::Deref,
	pin::Pin,
};

use futures_channel::oneshot;
use isoprenoid::runtime::{Propagation, SignalsRuntimeRef};
use pin_project::pin_project;

use crate::{
	opaque::Opaque,
	signal::Strong,
	signals_helper,
	traits::{UnmanagedSignal, UnmanagedSignalCell},
	unmanaged::{computed, folded, reduced},
	Guard, Signal, SignalArc,
};

/// [`Subscription`] after type-erasure.
pub type SubscriptionDyn<'a, T, SR> = Subscription<T, dyn 'a + UnmanagedSignal<T, SR>, SR>;

/// [`Subscription`] after cell-type-erasure.
pub type SubscriptionDynCell<'a, T, SR> = Subscription<T, dyn 'a + UnmanagedSignalCell<T, SR>, SR>;

/// Intrinsically-subscribing version of [`SignalArc`].  
/// Can be directly constructed but also converted to and from that type.
#[must_use = "Subscriptions are undone when dropped."]
pub struct Subscription<
	T: ?Sized + Send,
	S: ?Sized + UnmanagedSignal<T, SR>,
	SR: ?Sized + SignalsRuntimeRef,
> {
	pub(crate) subscribed: ManuallyDrop<Strong<T, S, SR>>,
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef> Deref
	for Subscription<T, S, SR>
{
	type Target = Signal<T, S, SR>;

	fn deref(&self) -> &Self::Target {
		&self.subscribed
	}
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Borrow<Signal<T, S, SR>> for Subscription<T, S, SR>
{
	fn borrow(&self) -> &Signal<T, S, SR> {
		self.subscribed.borrow()
	}
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef> Debug
	for Subscription<T, S, SR>
where
	T: Debug,
{
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		self.subscribed.clone_runtime_ref().run_detached(|| {
			f.debug_struct("SubscriptionSR")
				.field("(value)", &&**self.subscribed.read_exclusive_dyn())
				.finish_non_exhaustive()
		})
	}
}

unsafe impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Send for Subscription<T, S, SR>
{
}
unsafe impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Sync for Subscription<T, S, SR>
{
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef> Drop
	for Subscription<T, S, SR>
{
	fn drop(&mut self) {
		//FIXME: This has the right semantics of skipping `.unsubscribe()` when exclusive,
		//       but almost certainly isn't optimised well.
		let weak = self.subscribed.downgrade();
		unsafe {
			// SAFETY: Dropped only once, here.
			ManuallyDrop::drop(&mut self.subscribed);
		}
		if let Some(strong) = weak.upgrade() {
			// The managed `Signal` wasn't exclusive (so it wasn't purged from the signals runtime),
			// so decrement its subscription count.
			strong._managed().unsubscribe();
		}
	}
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef> Clone
	for Subscription<T, S, SR>
{
	fn clone(&self) -> Self {
		self.subscribed._managed().subscribe();
		Self {
			subscribed: self.subscribed.clone(),
		}
	}
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: SignalsRuntimeRef>
	Subscription<T, S, SR>
{
	/// Constructs a new [`Subscription`] from the given [`UnmanagedSignal`].
	///
	/// Subscribes to it intrinsically in the process.
	pub fn new(unmanaged: S) -> Self
	where
		S: Sized,
	{
		unmanaged.clone_runtime_ref().run_detached(|| {
			let strong = Strong::pin(unmanaged);
			strong._managed().subscribe();
			// Important: Wrap only after subscribing succeeds!
			//            If there's a panic, we still want to release the `Strong` but without calling `.unsubscribe()`.
			//            (Technically the `<Self as Drop>::drop` also avoids this, but that's extra work anyway.)
			Self {
				subscribed: ManuallyDrop::new(strong),
			}
		})
	}

	/// Unsubscribes the [`Subscription`], turning it into a [`SignalArc`] in the process.
	///
	/// The underlying [`Signal`] may remain subscribed-to due to other subscriptions.
	#[must_use = "Use `drop(self)` instead of converting first. Dropping directly can skip signal refreshes caused by `Propagation::FlushOut`."]
	pub fn unsubscribe(self) -> SignalArc<T, S, SR> {
		//FIXME: This could avoid refcounting up and down and at least some of the associated memory barriers.
		SignalArc {
			strong: (*self.subscribed).clone(),
		}
	} // Implicit drop(self) unsubscribes.
}

impl<T: ?Sized + Send, S: Sized + UnmanagedSignal<T, SR>, SR: SignalsRuntimeRef>
	Subscription<T, S, SR>
{
	/// Erases the (generally opaque) type parameter `S`, allowing the [`Subscription`] to
	/// be stored easily.
	pub fn into_dyn<'a>(self) -> SubscriptionDyn<'a, T, SR>
	where
		T: 'a,
		S: 'a,
		SR: 'a,
	{
		unsafe {
			let this = ManuallyDrop::new(self);
			SubscriptionDyn {
				subscribed: ManuallyDrop::new(this.subscribed.unsafe_copy().into_dyn()),
			}
		}
	}

	/// Erases the (generally opaque) type parameter `S`, allowing the
	/// cell-[`Subscription`] to be stored easily.
	pub fn into_dyn_cell<'a>(self) -> SubscriptionDynCell<'a, T, SR>
	where
		T: 'a,
		S: 'a + UnmanagedSignalCell<T, SR>,
		SR: 'a,
	{
		unsafe {
			let this = ManuallyDrop::new(self);
			SubscriptionDynCell {
				subscribed: ManuallyDrop::new(this.subscribed.unsafe_copy().into_dyn_cell()),
			}
		}
	}
}

impl<T: ?Sized + Send, S: Sized + UnmanagedSignalCell<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Subscription<T, S, SR>
{
	/// Obscures the cell API, allowing only reads and subscriptions.
	pub fn into_read_only<'a>(self) -> Subscription<T, impl 'a + UnmanagedSignal<T, SR>, SR>
	where
		S: 'a,
	{
		unsafe {
			//SAFETY: Prevents dropping of the original `Weak`,
			//        so that the net count doesn't change.
			let this = ManuallyDrop::new(self);
			Subscription {
				subscribed: ManuallyDrop::new(this.subscribed.unsafe_copy()),
			}
		}
	}
}

impl<'a, T: 'a + ?Sized + Send, SR: 'a + ?Sized + SignalsRuntimeRef>
	SubscriptionDynCell<'a, T, SR>
{
	/// Obscures the cell API, allowing only reads and subscriptions.
	///
	/// Since 0.1.2.
	pub fn into_read_only(self) -> SubscriptionDyn<'a, T, SR> {
		unsafe {
			//SAFETY: Prevents dropping of the original `Weak`,
			//        so that the net count doesn't change.
			let this = ManuallyDrop::new(self);
			Subscription {
				subscribed: ManuallyDrop::new(this.subscribed.unsafe_copy().into_read_only()),
			}
		}
	}
}

/// Secondary constructors.
///
/// # Omissions
///
/// The "uncached" and "distinct" versions of [`computed`](`computed()`) are
/// intentionally not wrapped here, as their behaviour may be unexpected at first glance.
///
/// You can still easily construct them as [`SignalArc`] and subscribe afterwards:
///
/// ```
/// # {
/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
/// use flourish::GlobalSignalsRuntime;
///
/// type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
///
/// // The closure runs once on subscription, but not to refresh `sub`!
/// // It re-runs with each access of its value through `SourcePin`, instead.
/// let sub_uncached = Signal::computed_uncached(|| ()).into_subscription();
///
/// // The closure re-runs on each refresh, even if the inputs are equal!
/// // However, dependent signals are only invalidated if the result changed.
/// let sub_distinct = Signal::distinct(|| ()).into_subscription();
/// # }
/// ```
impl<T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef> Subscription<T, Opaque, SR> {
	/// A simple cached computation.
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::GlobalSignalsRuntime;
	/// # type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
	/// # type Subscription<T, S> = flourish::Subscription<T, S, GlobalSignalsRuntime>;
	/// # let input = Signal::cell(1);
	/// Subscription::computed(|| input.get() + 1);
	/// # }
	/// ```
	///
	/// Wraps [`computed`](`computed()`).
	pub fn computed<'a>(
		fn_pin: impl 'a + Send + FnMut() -> T,
	) -> Subscription<T, impl 'a + Sized + UnmanagedSignal<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a + Default,
	{
		Subscription::new(computed(fn_pin, SR::default()))
	}

	/// A simple cached computation.
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::{GlobalSignalsRuntime, Signal, Subscription};
	/// # let input = Signal::cell_with_runtime(1, GlobalSignalsRuntime);
	/// Subscription::computed_with_runtime(|| input.get() + 1, input.clone_runtime_ref());
	/// # }
	/// ```
	///
	/// Wraps [`computed`](`computed()`).
	pub fn computed_with_runtime<'a>(
		fn_pin: impl 'a + Send + FnMut() -> T,
		runtime: SR,
	) -> Subscription<T, impl 'a + Sized + UnmanagedSignal<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a,
	{
		Subscription::new(computed(fn_pin, runtime))
	}

	/// The closure mutates the value and returns a [`Propagation`].
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::{GlobalSignalsRuntime, Propagation};
	/// # type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
	/// # type Subscription<T, S> = flourish::Subscription<T, S, GlobalSignalsRuntime>;
	/// # #[derive(Default, Clone)] struct Container;
	/// # impl Container { fn sort(&mut self) {} }
	/// # let input = Signal::cell(Container);
	/// Subscription::folded(Container::default(), move |value| {
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
	) -> Subscription<T, impl 'a + Sized + UnmanagedSignal<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a + Default,
	{
		Subscription::new(folded(init, fold_fn_pin, SR::default()))
	}

	/// The closure mutates the value and returns a [`Propagation`].
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::{GlobalSignalsRuntime, Propagation, Signal, Subscription};
	/// # #[derive(Default, Clone)] struct Container;
	/// # impl Container { fn sort(&mut self) {} }
	/// # let input = Signal::cell_with_runtime(Container, GlobalSignalsRuntime);
	/// Subscription::folded_with_runtime(Container::default(), |value| {
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
	) -> Subscription<T, impl 'a + Sized + UnmanagedSignal<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a,
	{
		Subscription::new(folded(init, fold_fn_pin, runtime))
	}

	/// `select_fn_pin` computes each value.
	/// `reduce_fn_pin` updates the current value with the next and returns a [`Propagation`].
	/// Dependencies are detected across both closures.
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::{GlobalSignalsRuntime, Propagation};
	/// # type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
	/// type Subscription<T, S> = flourish::Subscription<T, S, GlobalSignalsRuntime>;
	///
	/// # let input = Signal::cell(1);
	/// let lowest_settled = Subscription::reduced(
	/// 	|| input.get(),
	/// 	|value, next| if next < *value {
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
	) -> Subscription<T, impl 'a + Sized + UnmanagedSignal<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a + Default,
	{
		Subscription::new(reduced(select_fn_pin, reduce_fn_pin, SR::default()))
	}

	/// `select_fn_pin` computes each value.
	/// `reduce_fn_pin` updates the current value with the next and returns a [`Propagation`].
	/// Dependencies are detected across both closures.
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use flourish::{GlobalSignalsRuntime, Propagation, Signal, Subscription};
	/// # let input = Signal::cell_with_runtime(1, GlobalSignalsRuntime);
	/// let lowest_settled = Subscription::reduced_with_runtime(
	/// 	|| input.get(),
	/// 	|value, next| if next < *value {
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
	) -> Subscription<T, impl 'a + Sized + UnmanagedSignal<T, SR>, SR>
	where
		T: 'a + Sized,
		SR: 'a,
	{
		Subscription::new(reduced(select_fn_pin, reduce_fn_pin, runtime))
	}

	/// When awaited, subscribes to the given expressions but only returns [`Poll::Ready`](`core::task::Poll::Ready`)
	/// once `predicate_fn_pin` returns `true`.
	///
	/// Note that dependencies of `predicate_fn_pin` are tracked separately and
	/// do not cause `select_fn_pin` to re-run.
	///
	/// How to erase the closure types:
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use std::{future::Future, pin::{pin, Pin}};
	/// # use flourish::GlobalSignalsRuntime;
	/// # type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
	/// type Subscription<T, S> = flourish::Subscription<T, S, GlobalSignalsRuntime>;
	/// type SubscriptionDyn<'a, T> = flourish::SubscriptionDyn<'a, T, GlobalSignalsRuntime>;
	///
	/// # #[derive(Default, Clone, Copy)] struct Value;
	/// # let input = Signal::cell(Value);
	/// let f: Pin<&dyn Future<Output = SubscriptionDyn<_>>> = pin!(async {
	/// 	Subscription::skipped_while(|| input.get(), |_| true).await.into_dyn()
	/// });
	/// # }
	/// ```
	///
	/// It's fine to [`unsubscribe`](`Subscription::unsubscribe`):
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use std::{future::Future, pin::{pin, Pin}};
	/// # use flourish::GlobalSignalsRuntime;
	/// # type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
	/// type Subscription<T, S> = flourish::Subscription<T, S, GlobalSignalsRuntime>;
	/// type SignalArcDyn<'a, T> = flourish::SignalArcDyn<'a, T, GlobalSignalsRuntime>;
	///
	/// # #[derive(Default, Clone, Copy)] struct Value;
	/// # let input = Signal::cell(Value);
	/// let f: Pin<&dyn Future<Output = SignalArcDyn<_>>> = pin!(async {
	/// 	Subscription::skipped_while(|| input.get(), |_| true).await.unsubscribe().into_dyn()
	/// });
	/// # }
	/// ```
	pub fn skipped_while<'f, 'a: 'f>(
		select_fn_pin: impl 'a + Send + FnMut() -> T,
		predicate_fn_pin: impl 'f + Send + FnMut(&T) -> bool,
	) -> impl 'f + Send + Future<Output = Subscription<T, impl 'a + UnmanagedSignal<T, SR>, SR>>
	where
		T: 'a + Sized,
		SR: 'a + Default,
	{
		Self::skipped_while_with_runtime(select_fn_pin, predicate_fn_pin, SR::default())
	}

	/// When awaited, subscribes to the given expressions but only returns [`Poll::Ready`](`core::task::Poll::Ready`)
	/// once `predicate_fn_pin` returns `true`.
	///
	/// Note that dependencies of `predicate_fn_pin` are tracked separately and
	/// do not cause `select_fn_pin` to re-run.
	///
	/// How to erase the closure types:
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use std::{future::Future, pin::{pin, Pin}};
	/// # use flourish::{GlobalSignalsRuntime, Subscription, SubscriptionDyn};
	/// # type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
	/// # #[derive(Default, Clone, Copy)] struct Value;
	/// # let input = Signal::cell(Value);
	/// let f: Pin<&dyn Future<Output = SubscriptionDyn<_, _>>> = pin!(async {
	/// 	Subscription::skipped_while_with_runtime(|| input.get(), |_| true, GlobalSignalsRuntime)
	/// 		.await.into_dyn()
	/// });
	/// # }
	/// ```
	///
	/// It's fine to [`unsubscribe`](`Subscription::unsubscribe`):
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use std::{future::Future, pin::{pin, Pin}};
	/// # use flourish::{GlobalSignalsRuntime, Subscription, SignalArcDyn};
	/// # type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
	/// # #[derive(Default, Clone, Copy)] struct Value;
	/// # let input = Signal::cell(Value);
	/// let f: Pin<&dyn Future<Output = SignalArcDyn<_, _>>> = pin!(async {
	/// 	Subscription::skipped_while_with_runtime(|| input.get(), |_| true, GlobalSignalsRuntime)
	/// 		.await.unsubscribe().into_dyn()
	/// });
	/// # }
	/// ```
	pub fn skipped_while_with_runtime<'f, 'a: 'f>(
		select_fn_pin: impl 'a + Send + FnMut() -> T,
		mut predicate_fn_pin: impl 'f + Send + FnMut(&T) -> bool,
		runtime: SR,
	) -> impl 'f + Send + Future<Output = Subscription<T, impl 'a + UnmanagedSignal<T, SR>, SR>>
	where
		T: 'a + Sized,
		SR: 'a,
	{
		async {
			let sub = Subscription::computed_with_runtime(select_fn_pin, runtime.clone());
			{
				let (notify_ready, ready) = oneshot::channel();
				let mut notify = Some(notify_ready);
				signals_helper! {
					let effect = effect_with_runtime!({
						let sub = &sub;
						move || {
							if !predicate_fn_pin(&**sub.read_exclusive_dyn()) {
								notify.take().expect("Reached only once.").send(()).expect("Iff cancelled, then together.");
							}
						}
					}, drop, runtime);
				}
				ready.await.expect("Iff cancelled, then together.");
			}
			sub
		}
	}

	/// When awaited, subscribes to its inputs (from both closures) and resolves to a
	/// [`Subscription`] that settles only to values for which `predicate_fn_pin` returns `true`.
	///
	/// Dependencies are tracked together for both closures.
	///
	/// How to erase the closure types:
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use std::{future::Future, pin::{pin, Pin}};
	/// # use flourish::GlobalSignalsRuntime;
	/// # type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
	/// type Subscription<T, S> = flourish::Subscription<T, S, GlobalSignalsRuntime>;
	/// type SubscriptionDyn<'a, T> = flourish::SubscriptionDyn<'a, T, GlobalSignalsRuntime>;
	///
	/// # #[derive(Default, Clone, Copy)] struct Value;
	/// # let input = Signal::cell(Value);
	/// let f: Pin<&dyn Future<Output = SubscriptionDyn<_>>> = pin!(async {
	/// 	Subscription::filtered(|| input.get(), |_| false).await.into_dyn()
	/// });
	/// # }
	/// ```
	///
	/// Note that the constructed [`Signal`] will generally not observe inputs while [`unsubscribe`](`Subscription::unsubscribe`)d!
	pub fn filtered<'a>(
		fn_pin: impl 'a + Send + FnMut() -> T,
		predicate_fn_pin: impl 'a + Send + FnMut(&T) -> bool,
	) -> impl 'a + Send + Future<Output = Subscription<T, impl 'a + UnmanagedSignal<T, SR>, SR>>
	where
		T: 'a + Copy,
		SR: 'a + Default,
	{
		Self::filtered_with_runtime(fn_pin, predicate_fn_pin, SR::default())
	}

	/// When awaited, subscribes to its inputs (from both closures) and resolves to a
	/// [`Subscription`] that settles only to values for which `predicate_fn_pin` returns `true`.
	///
	/// Dependencies are tracked together for both closures.
	///
	/// How to erase the closure types:
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use std::{future::Future, pin::{pin, Pin}};
	/// # use flourish::{GlobalSignalsRuntime, Subscription, SubscriptionDyn};
	/// # type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
	/// # #[derive(Default, Clone, Copy)] struct Value;
	/// # let input = Signal::cell(Value);
	/// let f: Pin<&dyn Future<Output = SubscriptionDyn<_, _>>> = pin!(async {
	/// 	Subscription::filtered_with_runtime(|| input.get(), |_| false, GlobalSignalsRuntime)
	/// 		.await.into_dyn()
	/// });
	/// # }
	/// ```
	///
	/// Note that the constructed [`Signal`] will generally not observe inputs while [`unsubscribe`](`Subscription::unsubscribe`)d!
	pub fn filtered_with_runtime<'a>(
		mut fn_pin: impl 'a + Send + FnMut() -> T,
		mut predicate_fn_pin: impl 'a + Send + FnMut(&T) -> bool,
		runtime: SR,
	) -> impl 'a + Send + Future<Output = Subscription<T, impl 'a + UnmanagedSignal<T, SR>, SR>>
	where
		T: 'a + Copy,
		SR: 'a,
	{
		async {
			let (notify_initialized, initialized) = oneshot::channel();
			let mut notify_initialized = Some(notify_initialized);
			let sub = Subscription::folded_with_runtime(
				MaybeUninit::uninit(),
				{
					move |value| {
						let next = fn_pin();
						if predicate_fn_pin(&next) {
							match notify_initialized.take() {
								None => {
									*unsafe { value.assume_init_mut() } = next;
								}
								Some(notify_initialized) => {
									value.write(next);
									notify_initialized
										.send(())
										.expect("Iff cancelled, then together.");
								}
							}
							Propagation::Propagate
						} else {
							Propagation::Halt
						}
					}
				},
				runtime,
			);
			initialized.await.expect("Iff cancelled, then together.");

			unsafe { assume_init_subscription(sub) }
		}
	}

	/// When awaited, subscribes to its inputs and resolves to a [`Subscription`] that
	/// settles only to payloads of [`Some`] variants returned by `fn_pin`.
	///
	/// How to erase the closure type:
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use std::{future::Future, pin::{pin, Pin}};
	/// # use flourish::GlobalSignalsRuntime;
	/// type Subscription<T, S> = flourish::Subscription<T, S, GlobalSignalsRuntime>;
	/// type SubscriptionDyn<'a, T> = flourish::SubscriptionDyn<'a, T, GlobalSignalsRuntime>;
	///
	/// # #[derive(Clone, Copy)] struct Value;
	/// let f: Pin<&dyn Future<Output = SubscriptionDyn<Value>>> = pin!(async {
	/// 	Subscription::filter_mapped(|| None).await.into_dyn()
	/// });
	/// # }
	/// ```
	///
	/// Note that the constructed [`Signal`] will generally not observe inputs while [`unsubscribe`](`Subscription::unsubscribe`)d!
	pub fn filter_mapped<'a>(
		fn_pin: impl 'a + Send + FnMut() -> Option<T>,
	) -> impl 'a + Send + Future<Output = Subscription<T, impl 'a + UnmanagedSignal<T, SR>, SR>>
	where
		T: 'a + Copy,
		SR: 'a + Default,
	{
		Self::filter_mapped_with_runtime(fn_pin, SR::default())
	}

	/// When awaited, subscribes to its inputs and resolves to a [`Subscription`] that
	/// settles only to payloads of [`Some`] variants returned by `fn_pin`.
	///
	/// How to erase the closure types:
	///
	/// ```
	/// # {
	/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
	/// # use std::{future::Future, pin::{pin, Pin}};
	/// # use flourish::{GlobalSignalsRuntime, Subscription, SubscriptionDyn};
	/// # #[derive(Clone, Copy)] struct Value;
	/// let f: Pin<&dyn Future<Output = SubscriptionDyn<Value, _>>> = pin!(async {
	/// 	Subscription::filter_mapped_with_runtime(|| None, GlobalSignalsRuntime).await.into_dyn()
	/// });
	/// # }
	/// ```
	///
	/// Note that the constructed [`Signal`] will generally not observe inputs while [`unsubscribe`](`Subscription::unsubscribe`)d!
	pub fn filter_mapped_with_runtime<'a>(
		mut fn_pin: impl 'a + Send + FnMut() -> Option<T>,
		runtime: SR,
	) -> impl 'a + Send + Future<Output = Subscription<T, impl 'a + UnmanagedSignal<T, SR>, SR>>
	where
		T: 'a + Copy,
		SR: 'a,
	{
		async {
			let (notify_initialized, initialized) = oneshot::channel();
			let mut notify_initialized = Some(notify_initialized);
			let sub = Subscription::folded_with_runtime(
				MaybeUninit::uninit(),
				{
					move |value| {
						if let Some(next) = fn_pin() {
							match notify_initialized.take() {
								None => {
									*unsafe { value.assume_init_mut() } = next;
								}
								Some(notify_initialized) => {
									value.write(next);
									notify_initialized
										.send(())
										.expect("Iff cancelled, then together.");
								}
							}
							Propagation::Propagate
						} else {
							Propagation::Halt
						}
					}
				},
				runtime,
			);
			initialized.await.expect("Iff cancelled, then together.");

			unsafe { assume_init_subscription(sub) }
		}
	}
}

unsafe fn assume_init_subscription<
	T: ?Sized + Send + Copy,
	S: UnmanagedSignal<MaybeUninit<T>, SR>,
	SR: SignalsRuntimeRef,
>(
	sub: Subscription<MaybeUninit<T>, S, SR>,
) -> Subscription<T, impl UnmanagedSignal<T, SR>, SR> {
	#[pin_project]
	#[repr(transparent)]
	struct AbiShim<T: ?Sized>(#[pin] T);

	impl<T: Send + Copy, S: UnmanagedSignal<MaybeUninit<T>, SR>, SR: SignalsRuntimeRef>
		UnmanagedSignal<T, SR> for AbiShim<S>
	{
		fn touch(self: Pin<&Self>) {
			self.project_ref().0.touch()
		}

		fn get(self: Pin<&Self>) -> T
		where
			T: Sync + Copy,
		{
			unsafe { self.project_ref().0.get().assume_init() }
		}

		fn get_clone(self: Pin<&Self>) -> T
		where
			T: Sync + Clone,
		{
			unsafe { self.project_ref().0.get_clone().assume_init() }
		}

		fn get_clone_exclusive(self: Pin<&Self>) -> T
		where
			T: Clone,
		{
			unsafe { self.project_ref().0.get_clone_exclusive().assume_init() }
		}

		fn get_exclusive(self: Pin<&Self>) -> T
		where
			T: Copy,
		{
			unsafe { self.project_ref().0.get_exclusive().assume_init() }
		}

		fn read<'r>(self: Pin<&'r Self>) -> Self::Read<'r>
		where
			Self: Sized,
			T: 'r + Sync,
		{
			AbiShim(self.project_ref().0.read())
		}

		type Read<'r>
			= AbiShim<S::Read<'r>>
		where
			Self: 'r + Sized,
			T: 'r + Sync;

		fn read_exclusive<'r>(self: Pin<&'r Self>) -> Self::ReadExclusive<'r>
		where
			Self: Sized,
			T: 'r,
		{
			AbiShim(self.project_ref().0.read_exclusive())
		}

		type ReadExclusive<'r>
			= AbiShim<S::ReadExclusive<'r>>
		where
			Self: 'r + Sized,
			T: 'r;

		fn read_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Guard<T>>
		where
			T: 'r + Sync,
		{
			unsafe {
				//SAFETY: `MaybeUninit` is ABI-compatible with what it wraps.
				Box::from_raw(
					*(&Box::into_raw(self.project_ref().0.read_exclusive_dyn())
						as *const *mut dyn Guard<MaybeUninit<T>> as *const *mut dyn Guard<T>),
				)
			}
		}

		fn read_exclusive_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Guard<T>>
		where
			T: 'r,
		{
			unsafe {
				//SAFETY: `MaybeUninit` is ABI-compatible with what it wraps.
				Box::from_raw(
					*(&Box::into_raw(self.project_ref().0.read_exclusive_dyn())
						as *const *mut dyn Guard<MaybeUninit<T>> as *const *mut dyn Guard<T>),
				)
			}
		}

		fn clone_runtime_ref(&self) -> SR
		where
			SR: Sized,
		{
			self.0.clone_runtime_ref()
		}

		fn subscribe(self: Pin<&Self>) {
			self.project_ref().0.subscribe()
		}

		fn unsubscribe(self: Pin<&Self>) {
			self.project_ref().0.unsubscribe()
		}
	}

	impl<T: ?Sized + Send + Copy, G: ?Sized + Guard<MaybeUninit<T>>> Guard<T> for AbiShim<G> {}

	impl<T: ?Sized + Send + Copy, G: ?Sized + Deref<Target = MaybeUninit<T>>> Deref for AbiShim<G> {
		type Target = T;

		fn deref(&self) -> &Self::Target {
			unsafe { self.0.deref().assume_init_ref() }
		}
	}

	impl<T: ?Sized + Send + Copy, G: ?Sized + Borrow<MaybeUninit<T>>> Borrow<T> for AbiShim<G> {
		fn borrow(&self) -> &T {
			unsafe { self.0.borrow().assume_init_ref() }
		}
	}

	unsafe {
		//SAFETY: This may reinterpret a fat pointer, which skips over the `AbiShim` methods
		//        entirely, but that's fine since everything is fully ABI-compatible.
		(*(&(&ManuallyDrop::new(sub) as *const ManuallyDrop<Subscription<MaybeUninit<T>, S, SR>>)
			as *const *const ManuallyDrop<Subscription<MaybeUninit<T>, S, SR>>
			as *const *const Subscription<T, AbiShim<S>, SR>))
			.read()
	}
}
