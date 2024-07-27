use std::{
	fmt::Debug,
	future::Future,
	marker::{PhantomData, PhantomPinned},
	mem,
	pin::Pin,
	sync::{Arc, Mutex, Weak},
};

use futures_lite::FutureExt;
use isoprenoid::runtime::{
	CallbackTableTypes, GlobalSignalsRuntime, Propagation, SignalsRuntimeRef,
};

use crate::{
	opaque::Opaque,
	shadow_clone,
	traits::{Guard, Subscribable, UnmanagedSignal, UnmanagedSignalCell},
	unmanaged::{InertCell, ReactiveCell, ReactiveCellMut},
	ArcSignal, ArcSignalDyn, Signal, SignalDyn, SourcePin, UnmanagedSignalCellPin,
};

//TODO
// /// Type inference helper alias for [`ArcSignalCell`] (using [`GlobalSignalsRuntime`]).
// pub type SignalCell<T, S> = ArcSignalCell<T, S, GlobalSignalsRuntime>;

/// Type of [`ArcSignalCell`]s after type-erasure. Dynamic dispatch.
pub type ArcSignalCellDyn<'a, T, SR> = ArcSignalCell<T, dyn 'a + UnmanagedSignalCell<T, SR>, SR>;

/// Type of [`WeakSignalCell`]s after type-erasure or [`ArcSignalCellDyn`] after downgrade. Dynamic dispatch.
pub type WeakSignalCellDyn<'a, T, SR> = WeakSignalCell<T, dyn 'a + UnmanagedSignalCell<T, SR>, SR>;

pub struct WeakSignalCell<
	T: ?Sized + Send,
	S: ?Sized + UnmanagedSignalCell<T, SR>,
	SR: ?Sized + SignalsRuntimeRef,
> {
	pub(crate) weak: Weak<SignalCell<T, S, SR>>,
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignalCell<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	WeakSignalCell<T, S, SR>
{
	#[must_use]
	pub fn upgrade(&self) -> Option<ArcSignalCell<T, S, SR>> {
		self.source_cell.upgrade().map(|strong| ArcSignalCell {
			source_cell: unsafe { Pin::new_unchecked(strong) },
			upcast: self.upcast,
		})
	}
}

//TODO: Unwrapping.

pub struct ArcSignalCell<
	T: ?Sized + Send,
	S: ?Sized + UnmanagedSignalCell<T, SR>,
	SR: ?Sized + SignalsRuntimeRef,
> {
	pub(crate) arc: Arc<SignalCell<T, S, SR>>,
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignalCell<T, SR>, SR: ?Sized + SignalsRuntimeRef> Clone
	for ArcSignalCell<T, S, SR>
{
	fn clone(&self) -> Self {
		Self {
			source_cell: self.source_cell.clone(),
			upcast: self.upcast,
		}
	}
}

impl<
		T: ?Sized + Debug + Send,
		S: ?Sized + UnmanagedSignalCell<T, SR>,
		SR: SignalsRuntimeRef + Debug,
	> Debug for ArcSignalCell<T, S, SR>
where
	S: Debug,
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_tuple("SignalCell")
			.field(&self.source_cell)
			.finish()
	}
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct AssertSendSync<T: ?Sized>(pub(crate) T);
unsafe impl<T: ?Sized> Send for AssertSendSync<T> {}
unsafe impl<T: ?Sized> Sync for AssertSendSync<T> {}

impl<T> From<T> for AssertSendSync<T> {
	fn from(value: T) -> Self {
		Self(value)
	}
}

impl<T: Send, SR: SignalsRuntimeRef> ArcSignalCell<T, Opaque, SR> {
	pub fn new<'a>(
		initial_value: T,
	) -> ArcSignalCell<T, impl 'a + Sized + UnmanagedSignalCell<T, SR>, SR>
	where
		T: 'a,
		SR: 'a + Default,
	{
		Self::with_runtime(initial_value, SR::default())
	}

	pub fn with_runtime<'a>(
		initial_value: T,
		runtime: SR,
	) -> ArcSignalCell<T, impl 'a + Sized + UnmanagedSignalCell<T, SR>, SR>
	where
		T: 'a,
		SR: 'a + Default,
	{
		let arc = Arc::pin(InertCell::with_runtime(initial_value, runtime));
		ArcSignalCell {
			upcast: unsafe {
				let ptr = Arc::into_raw(Pin::into_inner_unchecked(Pin::clone(&arc)))
					as *const (dyn '_ + Subscribable<T, SR>);
				Arc::decrement_strong_count(ptr);
				ptr
			}
			.into(),
			source_cell: arc,
		}
	}

	pub fn new_cyclic<'a>(
		make_initial_value: impl 'a + FnOnce(WeakSignalCellDyn<'a, T, SR>) -> T,
	) -> ArcSignalCell<T, impl 'a + Sized + UnmanagedSignalCell<T, SR>, SR>
	where
		T: 'a,
		SR: 'a + Default,
	{
		Self::new_cyclic_with_runtime(make_initial_value, SR::default())
	}

	pub fn new_cyclic_with_runtime<'a>(
		make_initial_value: impl 'a + FnOnce(WeakSignalCellDyn<'a, T, SR>) -> T,
		runtime: SR,
	) -> ArcSignalCell<T, impl 'a + Sized + UnmanagedSignalCell<T, SR>, SR>
	where
		T: 'a,
		SR: 'a + Default,
	{
		let arc = unsafe {
			Pin::new_unchecked(Arc::new_cyclic(|weak| {
				InertCell::with_runtime(
					make_initial_value(WeakSignalCell {
						source_cell: weak.clone() as Weak<dyn 'a + UnmanagedSignalCell<T, SR>>,
						upcast: (weak.as_ptr() as *const dyn Subscribable<T, SR>).into(),
					}),
					runtime,
				)
			}))
		};
		ArcSignalCell {
			upcast: unsafe {
				let ptr = Arc::into_raw(Pin::into_inner_unchecked(Pin::clone(&arc)))
					as *const (dyn '_ + Subscribable<T, SR>);
				Arc::decrement_strong_count(ptr);
				ptr
			}
			.into(),
			source_cell: arc,
		}
	}

	pub fn new_reactive<
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
	) -> ArcSignalCell<T, impl 'a + Sized + UnmanagedSignalCell<T, SR>, SR>
	where
		T: 'a,
		SR: 'a + Default,
	{
		Self::new_reactive_with_runtime(initial_value, on_subscribed_change_fn_pin, SR::default())
	}

	pub fn new_reactive_with_runtime<
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
	) -> ArcSignalCell<T, impl 'a + Sized + UnmanagedSignalCell<T, SR>, SR>
	where
		T: 'a,
		SR: 'a + Default,
	{
		let arc = Arc::pin(ReactiveCell::with_runtime(
			initial_value,
			on_subscribed_change_fn_pin,
			runtime,
		));
		ArcSignalCell {
			upcast: unsafe {
				let ptr = Arc::into_raw(Pin::into_inner_unchecked(Pin::clone(&arc)))
					as *const (dyn '_ + Subscribable<T, SR>);
				Arc::decrement_strong_count(ptr);
				ptr
			}
			.into(),
			source_cell: arc,
		}
	}

	pub fn new_cyclic_reactive<
		'a,
		HandlerFnPin: 'a
			+ Send
			+ FnMut(
				&T,
				<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
			) -> Propagation,
	>(
		make_initial_value_and_on_subscribed_change_fn_pin: impl FnOnce(
			WeakSignalCellDyn<'a, T, SR>,
		) -> (T, HandlerFnPin),
	) -> ArcSignalCell<T, impl 'a + Sized + UnmanagedSignalCell<T, SR>, SR>
	where
		T: 'a,
		SR: 'a + Default,
	{
		Self::new_cyclic_reactive_with_runtime(
			make_initial_value_and_on_subscribed_change_fn_pin,
			SR::default(),
		)
	}

	pub fn new_cyclic_reactive_with_runtime<
		'a,
		HandlerFnPin: 'a
			+ Send
			+ FnMut(
				&T,
				<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
			) -> Propagation,
	>(
		make_initial_value_and_on_subscribed_change_fn_pin: impl FnOnce(
			WeakSignalCellDyn<'a, T, SR>,
		) -> (T, HandlerFnPin),
		runtime: SR,
	) -> ArcSignalCell<T, impl 'a + Sized + UnmanagedSignalCell<T, SR>, SR>
	where
		T: 'a,
		SR: 'a + Default,
	{
		let arc = unsafe {
			Pin::new_unchecked(Arc::new_cyclic(|weak| {
				let (initial_value, on_subscribed_change_fn_pin) =
					make_initial_value_and_on_subscribed_change_fn_pin(WeakSignalCell {
						source_cell: weak.clone() as Weak<dyn 'a + UnmanagedSignalCell<T, SR>>,
						upcast: (weak.as_ptr() as *const dyn Subscribable<T, SR>).into(),
					});
				ReactiveCell::with_runtime(initial_value, on_subscribed_change_fn_pin, runtime)
			}))
		};
		ArcSignalCell {
			upcast: unsafe {
				let ptr = Arc::into_raw(Pin::into_inner_unchecked(Pin::clone(&arc)))
					as *const (dyn '_ + Subscribable<T, SR>);
				Arc::decrement_strong_count(ptr);
				ptr
			}
			.into(),
			source_cell: arc,
		}
	}

	pub fn new_reactive_mut<'a>(
		initial_value: T,
		on_subscribed_change_fn_pin: impl 'a
			+ Send
			+ FnMut(
				&mut T,
				<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
			) -> Propagation,
	) -> ArcSignalCell<T, impl 'a + Sized + UnmanagedSignalCell<T, SR>, SR>
	where
		T: 'a,
		SR: 'a + Default,
	{
		Self::new_reactive_mut_with_runtime(
			initial_value,
			on_subscribed_change_fn_pin,
			SR::default(),
		)
	}

	pub fn new_reactive_mut_with_runtime<'a>(
		initial_value: T,
		on_subscribed_change_fn_pin: impl 'a
			+ Send
			+ FnMut(
				&mut T,
				<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
			) -> Propagation,
		runtime: SR,
	) -> ArcSignalCell<T, impl 'a + Sized + UnmanagedSignalCell<T, SR>, SR>
	where
		T: 'a,
		SR: 'a + Default,
	{
		let arc = Arc::pin(ReactiveCellMut::with_runtime(
			initial_value,
			on_subscribed_change_fn_pin,
			runtime,
		));
		ArcSignalCell {
			upcast: unsafe {
				let ptr = Arc::into_raw(Pin::into_inner_unchecked(Pin::clone(&arc)))
					as *const (dyn '_ + Subscribable<T, SR>);
				Arc::decrement_strong_count(ptr);
				ptr
			}
			.into(),
			source_cell: arc,
		}
	}

	pub fn new_cyclic_reactive_mut<
		'a,
		HandlerFnPin: 'a
			+ Send
			+ FnMut(
				&mut T,
				<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
			) -> Propagation,
	>(
		make_initial_value_and_on_subscribed_change_fn_pin: impl FnOnce(
			WeakSignalCellDyn<'a, T, SR>,
		) -> (T, HandlerFnPin),
	) -> ArcSignalCell<T, impl 'a + Sized + UnmanagedSignalCell<T, SR>, SR>
	where
		T: 'a,
		SR: 'a + Default,
	{
		Self::new_cyclic_reactive_mut_with_runtime(
			make_initial_value_and_on_subscribed_change_fn_pin,
			SR::default(),
		)
	}

	//TODO: Pinning versions of these constructors.
	pub fn new_cyclic_reactive_mut_with_runtime<
		'a,
		HandlerFnPin: 'a
			+ Send
			+ FnMut(
				&mut T,
				<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
			) -> Propagation,
	>(
		make_initial_value_and_on_subscribed_change_fn_pin: impl FnOnce(
			WeakSignalCellDyn<'a, T, SR>,
		) -> (T, HandlerFnPin),
		runtime: SR,
	) -> ArcSignalCell<T, impl 'a + Sized + UnmanagedSignalCell<T, SR>, SR>
	where
		T: 'a,
		HandlerFnPin: 'a,
		SR: 'a + Default,
	{
		let arc = unsafe {
			Pin::new_unchecked(Arc::new_cyclic(|weak| {
				let (initial_value, on_subscribed_change_fn_pin) =
					make_initial_value_and_on_subscribed_change_fn_pin(WeakSignalCell {
						source_cell: weak.clone() as Weak<dyn 'a + UnmanagedSignalCell<T, SR>>,
						upcast: (weak.as_ptr() as *const dyn Subscribable<T, SR>).into(),
					});
				ReactiveCellMut::with_runtime(initial_value, on_subscribed_change_fn_pin, runtime)
			}))
		};
		ArcSignalCell {
			upcast: unsafe {
				let ptr = Arc::into_raw(Pin::into_inner_unchecked(Pin::clone(&arc)))
					as *const (dyn '_ + Subscribable<T, SR>);
				Arc::decrement_strong_count(ptr);
				ptr
			}
			.into(),
			source_cell: arc,
		}
	}
}

impl<T: ?Sized + Send, S: Sized + UnmanagedSignalCell<T, SR>, SR: SignalsRuntimeRef>
	ArcSignalCell<T, S, SR>
{
	/// Cheaply creates a [`ArcSignal`] handle to the managed [`UnmanagedSignalCell`].
	pub fn to_signal(&self) -> ArcSignal<T, S, SR> {
		ArcSignal {
			source: Pin::clone(&self.source_cell),
			_phantom: PhantomData,
		}
	}

	/// Cheaply borrows this [`ArcSignalCell`] as [`Signal`], which is [`Copy`].
	pub fn as_signal_ref(&self) -> Signal<'_, T, S, SR> {
		Signal {
			source: unsafe {
				let ptr = Pin::clone(&self.source_cell);
				let ptr = Arc::into_raw(Pin::into_inner_unchecked(ptr));
				Arc::decrement_strong_count(ptr);
				ptr
			},
			_phantom: PhantomData,
		}
	}
}

impl<'a, T: 'a + ?Sized + Send, SR: 'a + SignalsRuntimeRef> ArcSignalCellDyn<'a, T, SR> {
	/// Cheaply creates a [`ArcSignalDyn`] handle to the managed [`UnmanagedSignalCell`].
	pub fn to_signal(&self) -> ArcSignalDyn<'a, T, SR> {
		ArcSignal {
			source: unsafe {
				Arc::increment_strong_count(self.upcast.0);
				Pin::new_unchecked(Arc::from_raw(self.upcast.0))
			},
			_phantom: PhantomData,
		}
	}

	/// Cheaply borrows this [`ArcSignalCellDyn`] as [`SignalDyn`], which is [`Copy`].
	pub fn as_signal_ref(&self) -> SignalDyn<'_, 'a, T, SR> {
		Signal {
			source: self.upcast.0,
			_phantom: PhantomData,
		}
	}
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignalCell<T, SR>, SR: SignalsRuntimeRef>
	ArcSignalCell<T, S, SR>
{
	pub fn downgrade(&self) -> WeakSignalCell<T, S, SR> {
		WeakSignalCell {
			source_cell: Arc::downgrade(unsafe {
				&Pin::into_inner_unchecked(Pin::clone(&self.source_cell))
			}),
			upcast: self.upcast,
		}
	}

	//TODO: Make this available for "`Dyn`"!
	pub fn into_signal_and_self(self) -> (ArcSignal<T, S, SR>, Self)
	where
		S: Sized,
	{
		(self.as_signal_ref().to_signal(), self)
	}

	//TODO: Make this available for "`Dyn`"!
	pub fn into_signal_and_self_dyn<'a>(
		self,
	) -> (ArcSignalDyn<'a, T, SR>, ArcSignalCellDyn<'a, T, SR>)
	where
		T: 'a,
		S: 'a + Sized,
		SR: 'a,
	{
		(self.to_signal().into_dyn(), self.into_dyn())
	}
}

impl<T: Send + ?Sized, S: Sized + UnmanagedSignalCell<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	SourcePin<T, SR> for ArcSignalCell<T, S, SR>
{
	fn touch(&self) {
		self.source_cell.as_ref().touch()
	}

	fn get_clone(&self) -> T
	where
		T: Sync + Clone,
	{
		self.source_cell.as_ref().get_clone()
	}

	fn get_clone_exclusive(&self) -> T
	where
		T: Clone,
	{
		self.source_cell.as_ref().get_clone_exclusive()
	}

	fn read<'r>(&'r self) -> S::Read<'r>
	where
		Self: Sized,
		T: 'r + Sync,
	{
		self.source_cell.as_ref().read()
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
		self.source_cell.as_ref().read_exclusive()
	}

	type ReadExclusive<'r> = S::ReadExclusive<'r>
	where
		Self: 'r + Sized,
		T: 'r;

	fn read_dyn<'r>(&'r self) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r + Sync,
	{
		UnmanagedSignal::read_dyn(self.source_cell.as_ref())
	}

	fn read_exclusive_dyn<'r>(&'r self) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r,
	{
		UnmanagedSignal::read_exclusive_dyn(self.source_cell.as_ref())
	}

	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		self.source_cell.as_ref().clone_runtime_ref()
	}
}

/// ⚠️ This implementation uses dynamic dispatch internally for all methods with `Self: Sized`
/// bound, which is a bit less performant than using those accessors without type erasure.
impl<'a, T: Send + ?Sized, SR: ?Sized + SignalsRuntimeRef> SourcePin<T, SR>
	for ArcSignalCellDyn<'a, T, SR>
{
	fn touch(&self) {
		self.source_cell.as_ref().touch()
	}

	fn get_clone(&self) -> T
	where
		T: Sync + Clone,
	{
		self.source_cell.as_ref().get_clone()
	}

	fn get_clone_exclusive(&self) -> T
	where
		T: Clone,
	{
		self.source_cell.as_ref().get_clone_exclusive()
	}

	fn read<'r>(&'r self) -> private::BoxedGuardDyn<'r, T>
	where
		Self: Sized,
		T: 'r + Sync,
	{
		private::BoxedGuardDyn(self.source_cell.as_ref().read_dyn())
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
		private::BoxedGuardDyn(self.source_cell.as_ref().read_exclusive_dyn())
	}

	type ReadExclusive<'r> = private::BoxedGuardDyn<'r, T>
	where
		Self: 'r + Sized,
		T: 'r;

	fn read_dyn<'r>(&'r self) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r + Sync,
	{
		self.source_cell.as_ref().read_dyn()
	}

	fn read_exclusive_dyn<'r>(&'r self) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r,
	{
		self.source_cell.as_ref().read_exclusive_dyn()
	}

	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		self.source_cell.as_ref().clone_runtime_ref()
	}
}

impl<T: Send + ?Sized, S: Sized + UnmanagedSignalCell<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	UnmanagedSignalCellPin<T, SR> for ArcSignalCell<T, S, SR>
{
	fn change(&self, new_value: T)
	where
		T: 'static + Sized + PartialEq,
	{
		self.source_cell.as_ref().change(new_value)
	}

	fn replace(&self, new_value: T)
	where
		T: 'static + Sized,
	{
		self.source_cell.as_ref().replace(new_value)
	}

	fn update(&self, update: impl 'static + Send + FnOnce(&mut T) -> Propagation)
	where
		Self: Sized,
		T: 'static,
	{
		self.source_cell.as_ref().update(update)
	}

	fn update_dyn(&self, update: Box<dyn 'static + Send + FnOnce(&mut T) -> Propagation>)
	where
		T: 'static,
	{
		self.source_cell.as_ref().update_dyn(update)
	}

	fn change_async<'f>(
		&self,
		new_value: T,
	) -> private::DetachedAsyncFuture<'f, Result<Result<T, T>, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized + PartialEq,
	{
		let this = self.downgrade();
		private::DetachedAsyncFuture(
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

	type ChangeAsync<'f> = private::DetachedAsyncFuture<'f, Result<Result<T,T>, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	fn replace_async<'f>(&self, new_value: T) -> private::DetachedAsyncFuture<'f, Result<T, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized,
	{
		let this = self.downgrade();
		private::DetachedAsyncFuture(
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

	type ReplaceAsync<'f> = private::DetachedAsyncFuture<'f, Result<T, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	fn update_async<'f, U: 'f + Send, F: 'f + Send + FnOnce(&mut T) -> (Propagation, U)>(
		&self,
		update: F,
	) -> private::DetachedAsyncFuture<'f, Result<U, F>>
	where
		Self: 'f + Sized,
	{
		let this = self.downgrade();
		private::DetachedAsyncFuture(
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

	type UpdateAsync<'f, U: 'f, F: 'f>=private::DetachedAsyncFuture<'f, Result<U, F>>
	where
		Self: 'f + Sized;

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
				this.change_eager(new_value).boxed().await
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
				this.replace_eager(new_value).boxed().await
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

	fn change_eager<'f>(&self, new_value: T) -> S::ChangeEager<'f>
	where
		Self: 'f + Sized,
		T: 'f + Sized + PartialEq,
	{
		self.source_cell.as_ref().change_eager(new_value)
	}

	type ChangeEager<'f> = S::ChangeEager<'f>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	fn replace_eager<'f>(&self, new_value: T) -> S::ReplaceEager<'f>
	where
		Self: 'f + Sized,
		T: Sized,
	{
		self.source_cell.as_ref().replace_eager(new_value)
	}

	type ReplaceEager<'f> = S::ReplaceEager<'f>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	fn update_eager<'f, U: Send, F: 'f + Send + FnOnce(&mut T) -> (Propagation, U)>(
		&self,
		update: F,
	) -> S::UpdateEager<'f, U, F>
	where
		Self: 'f + Sized,
	{
		self.source_cell.as_ref().update_eager(update)
	}

	type UpdateEager<'f, U: 'f, F: 'f> = S::UpdateEager<'f, U, F>
	where
		Self: 'f + Sized;

	fn change_eager_dyn<'f>(
		&self,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<Result<T, T>, T>>>
	where
		T: 'f + Sized + PartialEq,
	{
		self.source_cell.as_ref().change_eager_dyn(new_value)
	}

	fn replace_eager_dyn<'f>(
		&self,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<T, T>>>
	where
		T: 'f + Sized,
	{
		self.source_cell.as_ref().replace_eager_dyn(new_value)
	}

	fn update_eager_dyn<'f>(
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
		self.source_cell.as_ref().update_eager_dyn(update)
	}

	fn change_blocking(&self, new_value: T) -> Result<T, T>
	where
		T: Sized + PartialEq,
	{
		self.source_cell.as_ref().change_blocking(new_value)
	}

	fn replace_blocking(&self, new_value: T) -> T
	where
		T: Sized,
	{
		self.source_cell.as_ref().replace_blocking(new_value)
	}

	fn update_blocking<U>(&self, update: impl FnOnce(&mut T) -> (Propagation, U)) -> U
	where
		Self: Sized,
	{
		self.source_cell.as_ref().update_blocking(update)
	}

	fn update_blocking_dyn(&self, update: Box<dyn '_ + FnOnce(&mut T) -> Propagation>) {
		self.source_cell.as_ref().update_blocking_dyn(update)
	}
}

/// ⚠️ This implementation uses dynamic dispatch internally for all methods with `Self: Sized`
/// bound, which is a bit less performant than using those accessors without type erasure.
impl<'a, T: Send + ?Sized, SR: ?Sized + SignalsRuntimeRef> UnmanagedSignalCellPin<T, SR>
	for ArcSignalCellDyn<'a, T, SR>
{
	fn change(&self, new_value: T)
	where
		T: 'static + Sized + PartialEq,
	{
		self.source_cell.as_ref().change(new_value)
	}

	fn replace(&self, new_value: T)
	where
		T: 'static + Sized,
	{
		self.source_cell.as_ref().replace(new_value)
	}

	fn update(&self, update: impl 'static + Send + FnOnce(&mut T) -> Propagation)
	where
		Self: Sized,
		T: 'static,
	{
		self.source_cell.as_ref().update_dyn(Box::new(update))
	}

	fn update_dyn(&self, update: Box<dyn 'static + Send + FnOnce(&mut T) -> Propagation>)
	where
		T: 'static,
	{
		self.source_cell.as_ref().update_dyn(Box::new(update))
	}

	fn change_async<'f>(
		&self,
		new_value: T,
	) -> private2::DetachedAsyncFuture<'f, Result<Result<T, T>, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized + PartialEq,
	{
		let this = self.downgrade();
		private2::DetachedAsyncFuture(
			Box::pin(async move {
				if let Some(this) = this.upgrade() {
					this.change_eager(new_value).await
				} else {
					Err(new_value)
				}
			}),
			PhantomPinned,
		)
	}

	type ChangeAsync<'f> = private2::DetachedAsyncFuture<'f, Result<Result<T,T>, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	fn replace_async<'f>(&self, new_value: T) -> private2::DetachedAsyncFuture<'f, Result<T, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized,
	{
		let this = self.downgrade();
		private2::DetachedAsyncFuture(
			Box::pin(async move {
				if let Some(this) = this.upgrade() {
					this.replace_eager(new_value).await
				} else {
					Err(new_value)
				}
			}),
			PhantomPinned,
		)
	}

	type ReplaceAsync<'f> = private2::DetachedAsyncFuture<'f, Result<T, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	fn update_async<'f, U: 'f + Send, F: 'f + Send + FnOnce(&mut T) -> (Propagation, U)>(
		&self,
		update: F,
	) -> private2::DetachedAsyncFuture<'f, Result<U, F>>
	where
		Self: 'f + Sized,
	{
		let this = self.downgrade();
		private2::DetachedAsyncFuture(
			Box::pin(async move {
				if let Some(this) = this.upgrade() {
					this.update_eager(update).await
				} else {
					Err(update)
				}
			}),
			PhantomPinned,
		)
	}

	type UpdateAsync<'f, U: 'f, F: 'f>=private2::DetachedAsyncFuture<'f, Result<U, F>>
	where
		Self: 'f + Sized;

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
				this.change_eager(new_value).await
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
				this.replace_eager(new_value).await
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

	fn change_eager<'f>(
		&self,
		new_value: T,
	) -> private2::DetachedEagerFuture<'f, Result<Result<T, T>, T>>
	where
		Self: 'f + Sized,
		T: Sized + PartialEq,
	{
		private2::DetachedEagerFuture(
			self.source_cell.as_ref().change_eager_dyn(new_value).into(),
			PhantomPinned,
		)
	}

	type ChangeEager<'f> = private2::DetachedEagerFuture<'f, Result<Result<T, T>, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	fn replace_eager<'f>(&self, new_value: T) -> private2::DetachedEagerFuture<'f, Result<T, T>>
	where
		Self: 'f + Sized,
		T: Sized,
	{
		private2::DetachedEagerFuture(
			self.source_cell
				.as_ref()
				.replace_eager_dyn(new_value)
				.into(),
			PhantomPinned,
		)
	}

	type ReplaceEager<'f> = private2::DetachedEagerFuture<'f, Result<T, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	fn update_eager<'f, U: Send, F: 'f + Send + FnOnce(&mut T) -> (Propagation, U)>(
		&self,
		update: F,
	) -> private2::DetachedEagerFuture<'f, Result<U, F>>
	where
		Self: 'f + Sized,
	{
		let shelve = Arc::new(Mutex::new(Some(Err(update))));
		let f: Pin<Box<_>> = self
			.source_cell
			.as_ref()
			.update_eager_dyn(Box::new({
				let shelve = Arc::downgrade(&shelve);
				move |value| {
					if let Some(shelve) = shelve.upgrade() {
						let update = shelve
							.try_lock()
							.expect("unreachable")
							.take()
							.expect("unreachable")
							.map(|_| ())
							.unwrap_err();
						let (propagation, u) = update(value);
						assert!(shelve
							.try_lock()
							.expect("unreachable")
							.replace(Ok(u))
							.is_none());
						propagation
					} else {
						Propagation::Halt
					}
				}
			}))
			.into();
		private2::DetachedEagerFuture(
			Box::pin(async move {
				f.await.ok();
				Arc::into_inner(shelve)
					.expect("unreachable")
					.into_inner()
					.expect("can't be poisoned")
					.expect("can't be `None`")
			}),
			PhantomPinned,
		)
	}

	type UpdateEager<'f, U: 'f, F: 'f> = private2::DetachedEagerFuture<'f, Result<U, F>>
	where
		Self: 'f + Sized;

	fn change_eager_dyn<'f>(
		&self,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<Result<T, T>, T>>>
	where
		T: 'f + Sized + PartialEq,
	{
		self.source_cell.as_ref().change_eager_dyn(new_value)
	}

	fn replace_eager_dyn<'f>(
		&self,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<T, T>>>
	where
		T: 'f + Sized,
	{
		self.source_cell.as_ref().replace_eager_dyn(new_value)
	}

	fn update_eager_dyn<'f>(
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
		self.source_cell.as_ref().update_eager_dyn(update)
	}

	fn change_blocking(&self, new_value: T) -> Result<T, T>
	where
		T: Sized + PartialEq,
	{
		self.source_cell.as_ref().change_blocking(new_value)
	}

	fn replace_blocking(&self, new_value: T) -> T
	where
		T: Sized,
	{
		self.source_cell.as_ref().replace_blocking(new_value)
	}

	fn update_blocking<U>(&self, update: impl FnOnce(&mut T) -> (Propagation, U)) -> U
	where
		Self: Sized,
	{
		let shelve = Arc::new(Mutex::new(Some(Err(update))));
		self.source_cell.update_blocking_dyn(Box::new({
			shadow_clone!(shelve);
			move |value| {
				let update = shelve
					.try_lock()
					.expect("unreachable")
					.take()
					.expect("unreachable")
					.map(|_| ())
					.unwrap_err();
				let (propagation, u) = update(value);
				assert!(shelve
					.try_lock()
					.expect("unreachable")
					.replace(Ok(u))
					.is_none());
				propagation
			}
		}));
		Arc::into_inner(shelve)
			.expect("unreachable")
			.into_inner()
			.expect("can't be poisoned")
			.expect("can't be `None`")
			.map_err(|_| ())
			.expect("can't be `Err` anymore")
	}

	fn update_blocking_dyn(&self, update: Box<dyn '_ + FnOnce(&mut T) -> Propagation>) {
		self.source_cell.as_ref().update_blocking_dyn(update)
	}
}

/// Type of [`SignalCell`]s after type-erasure. Dynamic dispatch.
pub type SignalCellDyn<'a, T, SR> = SignalCell<T, dyn 'a + UnmanagedSignalCell<T, SR>, SR>;

/// A very cheap [`ArcSignalCell`]-like borrow that's [`Copy`].
///
/// Can be cloned into an additional [`ArcSignalCell`] and indirectly subscribed to.
#[derive(Debug)]
pub struct SignalCell<
	T: ?Sized + Send,
	S: ?Sized + Subscribable<T, SR>,
	SR: ?Sized + SignalsRuntimeRef,
> {
	/// FIXME: This is a workaround for [`trait_upcasting`](https://doc.rust-lang.org/beta/unstable-book/language-features/trait-upcasting.html)
	/// being unstable. Once that's stabilised, this field can be removed.
	pub(crate) upcast: *const dyn Subscribable<T, SR>,
	pub(crate) source_cell: S,
}

unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Send
	for SignalCell<T, S, SR>
{
}
unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Sync
	for SignalCell<T, S, SR>
{
}

/// Duplicated to avoid identities.
mod private {
	use std::{
		borrow::Borrow,
		future::Future,
		marker::PhantomPinned,
		ops::Deref,
		pin::Pin,
		task::{Context, Poll},
	};

	use futures_lite::FutureExt;
	use pin_project::pin_project;

	use crate::traits::Guard;

	#[must_use = "Async futures do nothing unless awaited."]
	#[pin_project]
	pub struct DetachedAsyncFuture<'f, Output: 'f>(
		pub(super) Pin<Box<dyn 'f + Send + Future<Output = Output>>>,
		/// For forwards-compatibility with TAITIT-enabled data inlining.
		#[pin]
		pub(super) PhantomPinned,
	);

	impl<'f, Output: 'f> Future for DetachedAsyncFuture<'f, Output> {
		type Output = Output;

		fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
			self.project().0.poll(cx)
		}
	}

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

/// Duplicated to avoid identities.
mod private2 {
	use std::{
		future::Future,
		marker::PhantomPinned,
		pin::Pin,
		task::{Context, Poll},
	};

	use futures_lite::FutureExt;
	use pin_project::pin_project;

	#[must_use = "Eager futures may still cancel their effect iff dropped."]
	#[pin_project]
	pub struct DetachedEagerFuture<'f, Output: 'f>(
		pub(super) Pin<Box<dyn 'f + Send + Future<Output = Output>>>,
		/// For forwards-compatibility with TAITIT-enabled data inlining.
		#[pin]
		pub(super) PhantomPinned,
	);

	impl<'f, Output: 'f> Future for DetachedEagerFuture<'f, Output> {
		type Output = Output;

		fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
			self.project().0.poll(cx)
		}
	}

	#[must_use = "Async futures do nothing unless awaited."]
	#[pin_project]
	pub struct DetachedAsyncFuture<'f, Output: 'f>(
		pub(super) Pin<Box<dyn 'f + Send + Future<Output = Output>>>,
		/// For forwards-compatibility with TAITIT-enabled data inlining.
		#[pin]
		pub(super) PhantomPinned,
	);

	impl<'f, Output: 'f> Future for DetachedAsyncFuture<'f, Output> {
		type Output = Output;

		fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
			self.project().0.poll(cx)
		}
	}
}
