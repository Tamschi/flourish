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
};

use futures_lite::FutureExt as _;
use isoprenoid::runtime::{CallbackTableTypes, Propagation, SignalsRuntimeRef};
use tap::Conv;

use crate::{
	opaque::Opaque,
	signal_arc::SignalWeakDynCell,
	traits::{Subscribable, UnmanagedSignalCell},
	unmanaged::{
		computed, computed_uncached, computed_uncached_mut, debounced, folded, reduced, InertCell,
		ReactiveCell, ReactiveCellMut,
	},
	Guard, SignalArc, SignalWeak, Subscription,
};

pub struct Signal<T: ?Sized + Send, S: ?Sized + Send + Sync, SR: ?Sized + SignalsRuntimeRef> {
	inner: UnsafeCell<Signal_<T, S, SR>>,
}

pub type SignalDyn<'a, T, SR> = Signal<T, dyn 'a + Subscribable<T, SR>, SR>;
pub type SignalDynCell<'a, T, SR> = Signal<T, dyn 'a + UnmanagedSignalCell<T, SR>, SR>;

impl<T: ?Sized + Send, S: ?Sized + Send + Sync, SR: ?Sized + SignalsRuntimeRef> Signal<T, S, SR> {
	fn inner(&self) -> &Signal_<T, S, SR> {
		unsafe { &*self.inner.get().cast_const() }
	}

	unsafe fn inner_mut(&mut self) -> &mut Signal_<T, S, SR> {
		self.inner.get_mut()
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Debug
	for Signal<T, S, SR>
where
	S: Debug,
{
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		f.debug_tuple("Signal")
			.field(&&*self.inner().managed)
			.finish()
	}
}

/// Secondary constructors.
impl<T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef> Signal<T, Opaque, SR> {
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

/// Cell constructors.
impl<T: Send, SR: SignalsRuntimeRef> Signal<T, Opaque, SR> {
	pub fn cell<'a>(
		initial_value: T,
	) -> SignalArc<T, impl 'a + Sized + UnmanagedSignalCell<T, SR>, SR>
	where
		T: 'a,
		SR: 'a + Default,
	{
		Self::cell_with_runtime(initial_value, SR::default())
	}

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

	pub fn cell_cyclic<'a>(
		make_initial_value: impl 'a + FnOnce(&SignalWeakDynCell<'a, T, SR>) -> T,
	) -> SignalArc<T, impl 'a + Sized + UnmanagedSignalCell<T, SR>, SR>
	where
		T: 'a,
		SR: 'a + Default,
	{
		Self::cell_cyclic_with_runtime(make_initial_value, SR::default())
	}

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

	pub fn cell_reactive<
		'a,
		HandlerFnPin: 'a
			+ Send
			+ FnMut(
				&T,
				<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
			) -> Propagation,
	>(
		initial_value: T,
		on_subscribed_change_fn_pin: HandlerFnPin,
	) -> SignalArc<T, impl 'a + Sized + UnmanagedSignalCell<T, SR>, SR>
	where
		T: 'a,
		SR: 'a + Default,
	{
		Self::cell_reactive_with_runtime(initial_value, on_subscribed_change_fn_pin, SR::default())
	}

	pub fn cell_reactive_with_runtime<
		'a,
		HandlerFnPin: 'a
			+ Send
			+ FnMut(
				&T,
				<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
			) -> Propagation,
	>(
		initial_value: T,
		on_subscribed_change_fn_pin: HandlerFnPin,
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

	//TODO: Pinning versions of these constructors.
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
	managed: ManuallyDrop<S>,
}

pub(crate) struct Strong<
	T: ?Sized + Send,
	S: ?Sized + Subscribable<T, SR>,
	SR: ?Sized + SignalsRuntimeRef,
> {
	strong: *const Signal<T, S, SR>,
}

pub(crate) struct Weak<
	T: ?Sized + Send,
	S: ?Sized + Subscribable<T, SR>,
	SR: ?Sized + SignalsRuntimeRef,
> {
	weak: *const Signal<T, S, SR>,
}

/// # Safety
///
/// [`Send`] and [`Sync`] bound on `S` are implied by [`Subscribable`].
unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Send
	for Signal<T, S, SR>
{
}

/// # Safety
///
/// [`Send`] and [`Sync`] bound on `S` are implied by [`Subscribable`].
unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Send
	for Signal_<T, S, SR>
{
}

/// # Safety
///
/// [`Send`] and [`Sync`] bound on `S` are implied by [`Subscribable`].
unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Send
	for Strong<T, S, SR>
{
}

/// # Safety
///
/// [`Send`] and [`Sync`] bound on `S` are implied by [`Subscribable`].
unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Send
	for Weak<T, S, SR>
{
}

/// # Safety
///
/// [`Send`] and [`Sync`] bound on `S` are implied by [`Subscribable`].
unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Sync
	for Signal<T, S, SR>
{
}

/// # Safety
///
/// [`Send`] and [`Sync`] bound on `S` are implied by [`Subscribable`].
unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Sync
	for Signal_<T, S, SR>
{
}

/// # Safety
///
/// [`Send`] and [`Sync`] bound on `S` are implied by [`Subscribable`].
unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Sync
	for Strong<T, S, SR>
{
}

/// # Safety
///
/// [`Send`] and [`Sync`] bound on `S` are implied by [`Subscribable`].
unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Sync
	for Weak<T, S, SR>
{
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
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
					managed: ManuallyDrop::new(managed),
				}
				.into(),
			})),
		}
	}
	pub(crate) fn pin_cyclic(constructor: impl FnOnce(&Weak<T, S, SR>) -> S) -> Self
	where
		S: Sized,
	{
		let weak: *mut Signal<T, MaybeUninit<S>, SR> = Box::into_raw(Box::new(Signal {
			inner: Signal_ {
				_phantom: PhantomData,
				strong: 0.into(),
				weak: 1.into(),
				managed: ManuallyDrop::new(MaybeUninit::<S>::uninit()),
			}
			.into(),
		}));

		let weak = unsafe {
			(*weak)
				._managed_mut()
				.write(constructor(&*ManuallyDrop::new(Weak { weak: weak.cast() })));
			weak.cast::<Signal<T, S, SR>>()
		};

		(*ManuallyDrop::new(Self { strong: weak })).clone()
	}

	pub(crate) fn _get(&self) -> &Signal<T, S, SR> {
		unsafe { &*self.strong }
	}

	unsafe fn get_mut(&mut self) -> &mut Signal<T, S, SR> {
		&mut *self.strong.cast_mut()
	}

	pub(crate) fn downgrade(&self) -> Weak<T, S, SR> {
		(*ManuallyDrop::new(Weak { weak: self.strong })).clone()
	}

	pub(crate) unsafe fn unsafe_copy(&self) -> Self {
		Self {
			strong: self.strong,
		}
	}

	pub(crate) fn into_dyn<'a>(self) -> Strong<T, dyn 'a + Subscribable<T, SR>, SR>
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

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Deref
	for Strong<T, S, SR>
{
	type Target = Signal<T, S, SR>;

	fn deref(&self) -> &Self::Target {
		self._get()
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Borrow<Signal<T, S, SR>> for Strong<T, S, SR>
{
	fn borrow(&self) -> &Signal<T, S, SR> {
		self._get()
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
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
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Drop
	for Strong<T, S, SR>
{
	fn drop(&mut self) {
		if self._get().inner().strong.fetch_sub(1, Ordering::Release) == 1 {
			unsafe { ManuallyDrop::drop(&mut self.get_mut().inner_mut().managed) }
			drop(Weak { weak: self.strong })
		}
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Drop
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

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> ToOwned
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

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Clone
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

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Clone
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
		unsafe { Pin::new_unchecked(&self.inner().managed) }
	}

	pub(crate) unsafe fn _managed_mut(&mut self) -> &mut S {
		&mut self.inner_mut().managed
	}
}

/// Management methods.
impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Signal<T, S, SR>
{
	pub fn subscribe(&self) -> Subscription<T, S, SR> {
		(*ManuallyDrop::new(Subscription {
			subscribed: Strong { strong: self },
		}))
		.clone()
	}

	pub fn downgrade(&self) -> SignalWeak<T, S, SR> {
		(*ManuallyDrop::new(SignalWeak {
			weak: Weak { weak: self },
		}))
		.clone()
	}

	pub fn as_dyn<'a>(&self) -> &SignalDyn<'a, T, SR>
	where
		S: 'a + Sized,
	{
		self
	}

	pub fn as_dyn_cell<'a>(&self) -> &SignalDynCell<'a, T, SR>
	where
		S: 'a + Sized + UnmanagedSignalCell<T, SR>,
	{
		self
	}
}

/// **Most application code should consume this.** Interface for movable signal handles that have an accessible value.
impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Signal<T, S, SR>
{
	/// Records `self` as dependency without accessing the value.
	pub fn touch(&self) {
		self._managed().touch()
	}

	/// Records `self` as dependency and retrieves a copy of the value.
	///
	/// Prefer [`SourcePin::touch`] where possible.
	pub fn get(&self) -> T
	where
		T: Sync + Copy,
	{
		self._managed().get()
	}

	/// Records `self` as dependency and retrieves a clone of the value.
	///
	/// Prefer [`SourcePin::get`] where available.
	pub fn get_clone(&self) -> T
	where
		T: Sync + Clone,
	{
		self._managed().get_clone()
	}

	/// Records `self` as dependency and retrieves a copy of the value.
	///
	/// Prefer [`SourcePin::get`] where available.
	pub fn get_exclusive(&self) -> T
	where
		T: Copy,
	{
		self._managed().get_clone_exclusive()
	}

	/// Records `self` as dependency and retrieves a clone of the value.
	///
	/// Prefer [`SourcePin::get_clone`] where available.
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
	/// Prefer [`SourcePin::read`] where available.
	pub fn read_exclusive<'r>(&'r self) -> S::ReadExclusive<'r>
	where
		S: Sized,
		T: 'r,
	{
		self._managed().read_exclusive()
	}

	/// The same as [`SourcePin::read`], but dyn-compatible.
	pub fn read_dyn<'r>(&'r self) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r + Sync,
	{
		self._managed().read_dyn()
	}

	/// The same as [`SourcePin::read_exclusive`], but dyn-compatible.
	///
	/// Prefer [`SourcePin::read_dyn`] where available.
	pub fn read_exclusive_dyn<'r>(&'r self) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r,
	{
		self._managed().read_exclusive_dyn()
	}

	/// Clones this [`SourcePin`]'s [`SignalsRuntimeRef`].
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
	pub fn change(&self, new_value: T)
	where
		T: 'static + Sized + PartialEq,
	{
		self._managed().change(new_value)
	}

	/// Unconditionally replaces the current value with `new_value` and signals dependents.
	///
	/// Prefer [`.change(new_value)`] if debouncing is acceptable.
	///
	/// # Logic
	///
	/// This method **must not** block *indefinitely*.  
	/// This method **may** defer its effect.
	pub fn replace(&self, new_value: T)
	where
		T: 'static + Sized,
	{
		self._managed().replace(new_value)
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

	/// Cheaply creates a [`Future`] that has the effect of [`change_eager`](`Signal::change_eager`) when polled.
	///
	/// # Logic
	///
	/// The [`Future`] *does not* hold a strong reference to `self`.
	fn change_async<'f>(&self, new_value: T) -> private::DetachedFuture<'f, Result<Result<T, T>, T>>
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
					this.change_eager(new_value).boxed().await
				} else {
					Err(new_value)
				}
			}),
			PhantomPinned,
		)
	}

	/// Cheaply creates a [`Future`] that has the effect of [`replace_eager`](`Signal::replace_eager`) when polled.
	///
	/// # Logic
	///
	/// The [`Future`] *does not* hold a strong reference to `self`.
	fn replace_async<'f>(&self, new_value: T) -> private::DetachedFuture<'f, Result<T, T>>
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
	///
	/// # Logic
	///
	/// The [`Future`] *does not* hold a strong reference to `self`.
	fn update_async<'f, U: 'f + Send, F: 'f + Send + FnOnce(&mut T) -> (Propagation, U)>(
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

	fn change_async_dyn<'f>(
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
				this.change_eager_dyn(new_value).conv::<Pin<Box<_>>>().await
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

	fn replace_async_dyn<'f>(
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

	fn update_async_dyn<'f>(
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
	pub fn change_eager<'f>(&self, new_value: T) -> S::ChangeEager<'f>
	where
		S: 'f + Sized,
		T: 'f + Sized + PartialEq,
	{
		self._managed().change_eager(new_value)
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

	/// The same as [`change_eager`](`Signal::change_eager`), but dyn-compatible.
	pub fn change_eager_dyn<'f>(
		&self,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<Result<T, T>, T>>>
	where
		T: 'f + Sized + PartialEq,
	{
		self._managed().change_eager_dyn(new_value)
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
	pub fn change_blocking(&self, new_value: T) -> Result<T, T>
	where
		T: Sized + PartialEq,
	{
		self._managed().change_blocking(new_value)
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
	pub(crate) struct DetachedFuture<'f, Output: 'f>(
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
