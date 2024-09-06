use std::{
	borrow::Borrow,
	fmt::{self, Debug, Formatter},
	future::Future,
	mem::{ManuallyDrop, MaybeUninit},
	ops::Deref,
	pin::Pin,
	sync::Arc,
};

use async_lock::OnceCell;
use isoprenoid::runtime::{Propagation, SignalsRuntimeRef};
use pin_project::pin_project;

use crate::{
	opaque::Opaque,
	shadow_clone,
	signal::Strong,
	signals_helper,
	traits::{Subscribable, UnmanagedSignal, UnmanagedSignalCell},
	unmanaged::{computed, folded, reduced},
	Guard, Signal, SignalArc,
};

/// Type of [`SubscriptionSR`]s after type-erasure. Dynamic dispatch.
pub type SubscriptionDyn<'a, T, SR> = Subscription<T, dyn 'a + Subscribable<T, SR>, SR>;

/// Type of [`SubscriptionSR`]s after type-erasure. Dynamic dispatch.
pub type SubscriptionDynCell<'a, T, SR> = Subscription<T, dyn 'a + UnmanagedSignalCell<T, SR>, SR>;

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
		self.subscribed.clone_runtime_ref().run_detached(|| {
			f.debug_struct("SubscriptionSR")
				.field("(value)", &&**self.subscribed.read_exclusive_dyn())
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
		self.subscribed._managed().unsubscribe();
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Clone
	for Subscription<T, S, SR>
{
	fn clone(&self) -> Self {
		self.subscribed._managed().subscribe();
		Self {
			subscribed: self.subscribed.clone(),
		}
	}
}

impl<
		'a,
		T: 'a + ?Sized + Send,
		S: 'a + Sized + Subscribable<T, SR>,
		SR: 'a + ?Sized + SignalsRuntimeRef,
	> From<Subscription<T, S, SR>> for SubscriptionDyn<'a, T, SR>
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
	pub fn into_dyn<'a>(self) -> SubscriptionDyn<'a, T, SR>
	where
		T: 'a,
		S: 'a,
		SR: 'a,
	{
		unsafe {
			let this = ManuallyDrop::new(self);
			SubscriptionDyn {
				subscribed: this.subscribed.unsafe_copy().into_dyn(),
			}
		}
	}

	pub fn into_dyn_cell<'a>(self) -> SubscriptionDynCell<'a, T, SR>
	where
		T: 'a,
		S: 'a + UnmanagedSignalCell<T, SR>,
		SR: 'a,
	{
		unsafe {
			let this = ManuallyDrop::new(self);
			SubscriptionDynCell {
				subscribed: this.subscribed.unsafe_copy().into_dyn_cell(),
			}
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
/// use flourish::GlobalSignalsRuntime;
///
/// type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
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

	/// When awaited, subscribes to the given expressions but only returns [`Poll::Ready`](`core::task::Poll::Ready`)
	/// once `predicate_fn_pin` returns `true`.
	///
	/// Note that while `predicate_fn_pin` is reactive (and automatically dependent on the
	/// resulting subscription), its exclusive dependencies do not cause `select_fn_pin`
	/// to re-run.
	pub fn skipped_while<'f, 'a: 'f>(
		select_fn_pin: impl 'a + Send + FnMut() -> T,
		predicate_fn_pin: impl 'f + Send + FnMut(&T) -> bool,
	) -> impl 'f + Send + Future<Output = Subscription<T, impl 'a + Subscribable<T, SR>, SR>>
	where
		T: 'a + Sized,
		SR: 'a + Default,
	{
		Self::skipped_while_with_runtime(select_fn_pin, predicate_fn_pin, SR::default())
	}

	/// When awaited, subscribes to the given expressions but only returns [`Poll::Ready`](`core::task::Poll::Ready`)
	/// once `predicate_fn_pin` returns `true`.
	///
	/// Note that while `predicate_fn_pin` is reactive (and automatically dependent on the
	/// resulting subscription), its exclusive dependencies do not cause `select_fn_pin`
	/// to re-run.
	pub fn skipped_while_with_runtime<'f, 'a: 'f>(
		select_fn_pin: impl 'a + Send + FnMut() -> T,
		mut predicate_fn_pin: impl 'f + Send + FnMut(&T) -> bool,
		runtime: SR,
	) -> impl 'f + Send + Future<Output = Subscription<T, impl 'a + Subscribable<T, SR>, SR>>
	where
		T: 'a + Sized,
		SR: 'a,
	{
		async {
			let sub = Subscription::computed_with_runtime(select_fn_pin, runtime.clone());
			{
				let once = OnceCell::<()>::new();
				signals_helper! {
						let effect = effect_with_runtime!({
						let (sub, once) = (&sub, &once);
						move || {
							if !predicate_fn_pin(&**sub.read_exclusive_dyn()) {
								once.set_blocking(()).ok();
							}
						}
					}, drop, runtime);
				}
				once.wait().await;
			}
			sub
		}
	}

	pub fn filtered<'a>(
		mut fn_pin: impl 'a + Send + FnMut() -> T,
		mut predicate_fn_pin: impl 'a + Send + FnMut(&T) -> bool,
	) -> impl 'a + Send + Future<Output = Subscription<T, impl 'a + Subscribable<T, SR>, SR>>
	where
		T: 'a + Copy,
		SR: 'a + Default,
	{
		Self::filtered_with_runtime(fn_pin, predicate_fn_pin, SR::default())
	}

	pub fn filtered_with_runtime<'a>(
		mut fn_pin: impl 'a + Send + FnMut() -> T,
		mut predicate_fn_pin: impl 'a + Send + FnMut(&T) -> bool,
		runtime: SR,
	) -> impl 'a + Send + Future<Output = Subscription<T, impl 'a + Subscribable<T, SR>, SR>>
	where
		T: 'a + Copy,
		SR: 'a,
	{
		async {
			// It's actually possible to avoid the `Arc` here, with a tri-state atomic or another `Once`,
			// since the closure is guaranteed to run when the subscription is created.
			// However, that would be considerably trickier code.
			let once = Arc::new(OnceCell::<()>::new());
			let sub = Subscription::folded_with_runtime(
				MaybeUninit::uninit(),
				{
					shadow_clone!(once);
					move |value| {
						let next = fn_pin();
						if predicate_fn_pin(&next) {
							if once.is_initialized() {
								*unsafe { value.assume_init_mut() } = next;
							} else {
								value.write(next);
								once.set_blocking(()).expect("unreachable");
							}
							Propagation::Propagate
						} else {
							Propagation::Halt
						}
					}
				},
				runtime,
			);
			once.wait().await;

			unsafe { assume_init_subscription(sub) }
		}
	}

	pub fn filter_mapped<'a>(
		mut fn_pin: impl 'a + Send + FnMut() -> Option<T>,
	) -> impl 'a + Send + Future<Output = Subscription<T, impl 'a + Subscribable<T, SR>, SR>>
	where
		T: 'a + Copy,
		SR: 'a + Default,
	{
		Self::filter_mapped_with_runtime(fn_pin, SR::default())
	}

	pub fn filter_mapped_with_runtime<'a>(
		mut fn_pin: impl 'a + Send + FnMut() -> Option<T>,
		runtime: SR,
	) -> impl 'a + Send + Future<Output = Subscription<T, impl 'a + Subscribable<T, SR>, SR>>
	where
		T: 'a + Copy,
		SR: 'a,
	{
		async {
			// It's actually possible to avoid the `Arc` here, with a tri-state atomic or another `Once`,
			// since the closure is guaranteed to run when the subscription is created.
			// However, that would be considerably trickier code.
			let once = Arc::new(OnceCell::<()>::new());
			let sub = Subscription::folded_with_runtime(
				MaybeUninit::uninit(),
				{
					shadow_clone!(once);
					move |value| {
						if let Some(next) = fn_pin() {
							if once.is_initialized() {
								*unsafe { value.assume_init_mut() } = next;
							} else {
								value.write(next);
								once.set_blocking(()).expect("unreachable");
							}
							Propagation::Propagate
						} else {
							Propagation::Halt
						}
					}
				},
				runtime,
			);
			once.wait().await;

			unsafe { assume_init_subscription(sub) }
		}
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

unsafe fn assume_init_subscription<
	T: ?Sized + Send + Copy,
	S: Subscribable<MaybeUninit<T>, SR>,
	SR: SignalsRuntimeRef,
>(
	sub: Subscription<MaybeUninit<T>, S, SR>,
) -> Subscription<T, impl Subscribable<T, SR>, SR> {
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

		type Read<'r> = AbiShim<S::Read<'r>>
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

		type ReadExclusive<'r> = AbiShim<S::ReadExclusive<'r>>
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
	}

	impl<T: Send + Copy, S: Subscribable<MaybeUninit<T>, SR>, SR: SignalsRuntimeRef>
		Subscribable<T, SR> for AbiShim<S>
	{
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
