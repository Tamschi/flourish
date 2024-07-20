use std::{
	borrow::Borrow,
	fmt::Debug,
	future::Future,
	marker::PhantomData,
	mem,
	ops::Deref,
	pin::Pin,
	sync::{Arc, Mutex, Weak},
};

use isoprenoid::runtime::{CallbackTableTypes, GlobalSignalRuntime, Propagation, SignalRuntimeRef};

use crate::{
	raw::{InertCell, ReactiveCell, ReactiveCellMut},
	traits::{Source, SourceCell, Subscribable},
	SignalRef, SignalSR, SourceCellPin, SourcePin,
};

/// Type inference helper alias for [`SignalCellSR`] (using [`GlobalSignalRuntime`]).
pub type SignalCell<T, S> = SignalCellSR<T, S, GlobalSignalRuntime>;

//TODO: It may be possible to fully implement the API with additional `dyn` methods on the traits.
/// Type of [`SignalCellSR`]s after type-erasure. Less convenient API.
pub type ErasedSignalCell<'a, T, SR> = SignalCellSR<T, dyn 'a + SourceCell<T, SR>, SR>;

pub type ErasedWeakSignalCell<'a, T, SR> = WeakSignalCell<T, dyn 'a + SourceCell<T, SR>, SR>;

pub struct WeakSignalCell<
	T: ?Sized + Send,
	S: ?Sized + SourceCell<T, SR>,
	SR: ?Sized + SignalRuntimeRef<Symbol: Sync>,
> {
	source_cell: Weak<S>,
	/// FIXME: This is a workaround for [`trait_upcasting`](https://doc.rust-lang.org/beta/unstable-book/language-features/trait-upcasting.html)
	/// being unstable. Once that's stabilised, this field can be removed.
	upcast: AssertSendSync<*const dyn Subscribable<SR, Output = T>>,
}

impl<
		T: ?Sized + Send,
		S: ?Sized + SourceCell<T, SR>,
		SR: ?Sized + SignalRuntimeRef<Symbol: Sync>,
	> WeakSignalCell<T, S, SR>
{
	#[must_use]
	pub fn upgrade(&self) -> Option<SignalCellSR<T, S, SR>> {
		self.source_cell.upgrade().map(|strong| SignalCellSR {
			source_cell: unsafe { Pin::new_unchecked(strong) },
			upcast: self.upcast,
		})
	}
}

pub struct SignalCellSR<
	T: ?Sized + Send,
	S: ?Sized + SourceCell<T, SR>,
	SR: ?Sized + SignalRuntimeRef<Symbol: Sync>,
> {
	source_cell: Pin<Arc<S>>,
	/// FIXME: This is a workaround for [`trait_upcasting`](https://doc.rust-lang.org/beta/unstable-book/language-features/trait-upcasting.html)
	/// being unstable. Once that's stabilised, this field can be removed.
	upcast: AssertSendSync<*const dyn Subscribable<SR, Output = T>>,
}

impl<
		T: ?Sized + Send,
		S: ?Sized + SourceCell<T, SR>,
		SR: ?Sized + SignalRuntimeRef<Symbol: Sync>,
	> Clone for SignalCellSR<T, S, SR>
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
		S: ?Sized + SourceCell<T, SR>,
		SR: SignalRuntimeRef<Symbol: Sync> + Debug,
	> Debug for SignalCellSR<T, S, SR>
where
	S: Debug,
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_tuple("SignalCell")
			.field(&self.source_cell)
			.finish()
	}
}

#[derive(Clone, Copy)]
struct AssertSendSync<T: ?Sized>(T);
unsafe impl<T: ?Sized> Send for AssertSendSync<T> {}
unsafe impl<T: ?Sized> Sync for AssertSendSync<T> {}

impl<T> From<T> for AssertSendSync<T> {
	fn from(value: T) -> Self {
		Self(value)
	}
}

impl<T: Send, SR: SignalRuntimeRef<Symbol: Sync>> SignalCellSR<T, InertCell<T, SR>, SR> {
	pub fn new(initial_value: T) -> Self
	where
		SR: Default,
	{
		Self::with_runtime(initial_value, SR::default())
	}

	pub fn with_runtime(initial_value: T, runtime: SR) -> Self
	where
		SR: Default,
	{
		let arc = Arc::pin(InertCell::with_runtime(initial_value, runtime));
		Self {
			upcast: unsafe {
				let ptr = Arc::into_raw(Pin::into_inner_unchecked(Pin::clone(&arc)))
					as *const (dyn '_ + Subscribable<SR, Output = T>);
				Arc::decrement_strong_count(ptr);
				ptr
			}
			.into(),
			source_cell: arc,
		}
	}

	pub fn new_cyclic<'a>(
		make_initial_value: impl FnOnce(ErasedWeakSignalCell<'a, T, SR>) -> T,
	) -> Self
	where
		T: 'a,
		SR: 'a + Default,
	{
		Self::new_cyclic_with_runtime(make_initial_value, SR::default())
	}

	pub fn new_cyclic_with_runtime<'a>(
		make_initial_value: impl FnOnce(ErasedWeakSignalCell<'a, T, SR>) -> T,
		runtime: SR,
	) -> Self
	where
		T: 'a,
		SR: 'a + Default,
	{
		let arc = unsafe {
			Pin::new_unchecked(Arc::new_cyclic(|weak| {
				InertCell::with_runtime(
					make_initial_value(WeakSignalCell {
						source_cell: weak.clone() as Weak<dyn 'a + SourceCell<T, SR>>,
						upcast: (weak.as_ptr() as *const dyn Subscribable<SR, Output = T>).into(),
					}),
					runtime,
				)
			}))
		};
		Self {
			upcast: unsafe {
				let ptr = Arc::into_raw(Pin::into_inner_unchecked(Pin::clone(&arc)))
					as *const (dyn '_ + Subscribable<SR, Output = T>);
				Arc::decrement_strong_count(ptr);
				ptr
			}
			.into(),
			source_cell: arc,
		}
	}

	pub fn read<'r>(&'r self) -> impl 'r + Borrow<T>
	where
		T: Sync,
	{
		self.source_cell.as_ref().read()
	}

	pub fn read_exclusive<'r>(&'r self) -> impl 'r + Borrow<T> {
		self.source_cell.as_ref().read_exclusive()
	}
}

// TODO: Make `HandlerFnPin` return `Update`, combined propagation!
impl<
		T: Send,
		HandlerFnPin: Send
			+ FnMut(
				&T,
				<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
			) -> Propagation,
		SR: SignalRuntimeRef<Symbol: Sync>,
	> SignalCellSR<T, ReactiveCell<T, HandlerFnPin, SR>, SR>
{
	pub fn new_reactive(initial_value: T, on_subscribed_change_fn_pin: HandlerFnPin) -> Self
	where
		SR: Default,
	{
		Self::new_reactive_with_runtime(initial_value, on_subscribed_change_fn_pin, SR::default())
	}

	pub fn new_reactive_with_runtime(
		initial_value: T,
		on_subscribed_change_fn_pin: HandlerFnPin,
		runtime: SR,
	) -> Self
	where
		SR: Default,
	{
		let arc = Arc::pin(ReactiveCell::with_runtime(
			initial_value,
			on_subscribed_change_fn_pin,
			runtime,
		));
		Self {
			upcast: unsafe {
				let ptr = Arc::into_raw(Pin::into_inner_unchecked(Pin::clone(&arc)))
					as *const (dyn '_ + Subscribable<SR, Output = T>);
				Arc::decrement_strong_count(ptr);
				ptr
			}
			.into(),
			source_cell: arc,
		}
	}

	pub fn new_cyclic_reactive<'a>(
		make_initial_value_and_on_subscribed_change_fn_pin: impl FnOnce(
			ErasedWeakSignalCell<'a, T, SR>,
		) -> (T, HandlerFnPin),
	) -> Self
	where
		T: 'a,
		HandlerFnPin: 'a,
		SR: 'a + Default,
	{
		Self::new_cyclic_reactive_with_runtime(
			make_initial_value_and_on_subscribed_change_fn_pin,
			SR::default(),
		)
	}

	pub fn new_cyclic_reactive_with_runtime<'a>(
		make_initial_value_and_on_subscribed_change_fn_pin: impl FnOnce(
			ErasedWeakSignalCell<'a, T, SR>,
		) -> (T, HandlerFnPin),
		runtime: SR,
	) -> Self
	where
		T: 'a,
		HandlerFnPin: 'a,
		SR: 'a + Default,
	{
		let arc = unsafe {
			Pin::new_unchecked(Arc::new_cyclic(|weak| {
				let (initial_value, on_subscribed_change_fn_pin) =
					make_initial_value_and_on_subscribed_change_fn_pin(WeakSignalCell {
						source_cell: weak.clone() as Weak<dyn 'a + SourceCell<T, SR>>,
						upcast: (weak.as_ptr() as *const dyn Subscribable<SR, Output = T>).into(),
					});
				ReactiveCell::with_runtime(initial_value, on_subscribed_change_fn_pin, runtime)
			}))
		};
		Self {
			upcast: unsafe {
				let ptr = Arc::into_raw(Pin::into_inner_unchecked(Pin::clone(&arc)))
					as *const (dyn '_ + Subscribable<SR, Output = T>);
				Arc::decrement_strong_count(ptr);
				ptr
			}
			.into(),
			source_cell: arc,
		}
	}

	pub fn read<'r>(&'r self) -> impl 'r + Borrow<T>
	where
		T: Sync,
	{
		self.source_cell.as_ref().read()
	}

	pub fn read_exclusive<'r>(&'r self) -> impl 'r + Borrow<T> {
		self.source_cell.as_ref().read_exclusive()
	}
}

// TODO: Make `HandlerFnPin` return `Update`, combined propagation!
impl<
		T: Send,
		HandlerFnPin: Send
			+ FnMut(
				&mut T,
				<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
			) -> Propagation,
		SR: SignalRuntimeRef<Symbol: Sync>,
	> SignalCellSR<T, ReactiveCellMut<T, HandlerFnPin, SR>, SR>
{
	pub fn new_reactive_mut(initial_value: T, on_subscribed_change_fn_pin: HandlerFnPin) -> Self
	where
		SR: Default,
	{
		Self::new_reactive_mut_with_runtime(
			initial_value,
			on_subscribed_change_fn_pin,
			SR::default(),
		)
	}

	pub fn new_reactive_mut_with_runtime(
		initial_value: T,
		on_subscribed_change_fn_pin: HandlerFnPin,
		runtime: SR,
	) -> Self
	where
		SR: Default,
	{
		let arc = Arc::pin(ReactiveCellMut::with_runtime(
			initial_value,
			on_subscribed_change_fn_pin,
			runtime,
		));
		Self {
			upcast: unsafe {
				let ptr = Arc::into_raw(Pin::into_inner_unchecked(Pin::clone(&arc)))
					as *const (dyn '_ + Subscribable<SR, Output = T>);
				Arc::decrement_strong_count(ptr);
				ptr
			}
			.into(),
			source_cell: arc,
		}
	}

	pub fn new_cyclic_reactive_mut<'a>(
		make_initial_value_and_on_subscribed_change_fn_pin: impl FnOnce(
			ErasedWeakSignalCell<'a, T, SR>,
		) -> (T, HandlerFnPin),
	) -> Self
	where
		T: 'a,
		HandlerFnPin: 'a,
		SR: 'a + Default,
	{
		Self::new_cyclic_reactive_mut_with_runtime(
			make_initial_value_and_on_subscribed_change_fn_pin,
			SR::default(),
		)
	}

	pub fn new_cyclic_reactive_mut_with_runtime<'a>(
		make_initial_value_and_on_subscribed_change_fn_pin: impl FnOnce(
			ErasedWeakSignalCell<'a, T, SR>,
		) -> (T, HandlerFnPin),
		runtime: SR,
	) -> Self
	where
		T: 'a,
		HandlerFnPin: 'a,
		SR: 'a + Default,
	{
		let arc = unsafe {
			Pin::new_unchecked(Arc::new_cyclic(|weak| {
				let (initial_value, on_subscribed_change_fn_pin) =
					make_initial_value_and_on_subscribed_change_fn_pin(WeakSignalCell {
						source_cell: weak.clone() as Weak<dyn 'a + SourceCell<T, SR>>,
						upcast: (weak.as_ptr() as *const dyn Subscribable<SR, Output = T>).into(),
					});
				ReactiveCellMut::with_runtime(initial_value, on_subscribed_change_fn_pin, runtime)
			}))
		};
		Self {
			upcast: unsafe {
				let ptr = Arc::into_raw(Pin::into_inner_unchecked(Pin::clone(&arc)))
					as *const (dyn '_ + Subscribable<SR, Output = T>);
				Arc::decrement_strong_count(ptr);
				ptr
			}
			.into(),
			source_cell: arc,
		}
	}

	pub fn read<'r>(&'r self) -> impl 'r + Borrow<T>
	where
		T: Sync,
	{
		self.source_cell.as_ref().read()
	}

	pub fn read_exclusive<'r>(&'r self) -> impl 'r + Borrow<T> {
		self.source_cell.as_ref().read_exclusive()
	}
}

impl<T: Send, S: ?Sized + SourceCell<T, SR>, SR: SignalRuntimeRef<Symbol: Sync>>
	SignalCellSR<T, S, SR>
{
	//TODO: `as_ref`/`SignalCellRef`?

	/// Cheaply borrows this [`SignalCell`] as [`SignalRef`], which is [`Copy`].
	pub fn as_signal_ref<'a>(&self) -> SignalRef<'_, 'a, T, SR>
	where
		T: 'a,
		SR: 'a,
	{
		SignalRef {
			source: self.upcast.0,
			_phantom: PhantomData,
		}
	}

	/// Cheaply creates a [`SignalSR`] handle to the managed [`SourceCell`].
	pub fn to_signal<'a>(&self) -> SignalSR<'a, T, SR>
	where
		T: 'a,
		S: 'a + Sized,
		SR: 'a,
	{
		SignalSR {
			source: Pin::clone(&self.source_cell) as Pin<Arc<dyn Subscribable<SR, Output = T>>>,
		}
	}

	pub fn into_erased<'a>(self) -> ErasedSignalCell<'a, T, SR>
	where
		S: 'a + Sized,
	{
		SignalCellSR {
			source_cell: self.source_cell,
			upcast: self.upcast,
		}
	}

	pub fn downgrade(&self) -> WeakSignalCell<T, S, SR> {
		WeakSignalCell {
			source_cell: Arc::downgrade(unsafe {
				&Pin::into_inner_unchecked(Pin::clone(&self.source_cell))
			}),
			upcast: self.upcast,
		}
	}

	pub fn into_signal_and_self<'a>(self) -> (SignalSR<'a, T, SR>, Self)
	where
		S: 'a + Sized,
	{
		(self.as_signal_ref().to_signal(), self)
	}

	pub fn into_signal_and_erased<'a>(self) -> (SignalSR<'a, T, SR>, ErasedSignalCell<'a, T, SR>)
	where
		S: 'a + Sized,
	{
		(self.as_signal_ref().to_signal(), self.into_erased())
	}
}

//TODO: Clean up `Sync: Sync`… everywhere.
impl<
		T: Send + Sized + ?Sized,
		S: ?Sized + SourceCell<T, SR>,
		SR: ?Sized + SignalRuntimeRef<Symbol: Sync>,
	> SourcePin<SR> for SignalCellSR<T, S, SR>
{
	type Output = T;

	fn touch(&self) {
		self.source_cell.as_ref().touch()
	}

	fn get_clone(&self) -> Self::Output
	where
		Self::Output: Sync + Clone,
	{
		self.source_cell.as_ref().get_clone()
	}

	fn get_clone_exclusive(&self) -> Self::Output
	where
		Self::Output: Clone,
	{
		self.source_cell.as_ref().get_clone_exclusive()
	}

	fn read<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Output>>
	where
		Self::Output: 'r + Sync,
	{
		Source::read(self.source_cell.as_ref())
	}

	fn read_exclusive<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Output>> {
		Source::read_exclusive(self.source_cell.as_ref())
	}

	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		self.source_cell.as_ref().clone_runtime_ref()
	}
}

impl<T: Send + Sized + ?Sized, S: Sized + SourceCell<T, SR>, SR: ?Sized + SignalRuntimeRef>
	SourceCellPin<T, SR> for SignalCellSR<T, S, SR>
where
	<SR as SignalRuntimeRef>::Symbol: Sync,
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
		<SR as SignalRuntimeRef>::Symbol: Sync,
	{
		self.source_cell.as_ref().update(update)
	}

	fn change_eager<'f>(&self, new_value: T) -> private::DetachedFuture<'f, Result<Result<T, T>, T>>
	where
		Self: 'f + Sized,
		T: Sized + PartialEq,
	{
		let this = self.downgrade();
		async move {
			let r = Arc::new(Mutex::new(Some(Err(new_value))));
			if let Some(this) = this.upgrade() {
				let f = this.update_async({
					let r = Arc::downgrade(&r);
					move |value| {
						let Some(r) = r.upgrade() else {
							return (Propagation::Halt, ());
						};
						let mut r = r.try_lock().unwrap();
						let new_value = r.take().unwrap().map(|_| ()).unwrap_err();
						if *value != new_value {
							*r = Some(Ok(Ok(mem::replace(value, new_value))));
							(Propagation::Propagate, ())
						} else {
							*r = Some(Ok(Err(new_value)));
							(Propagation::Halt, ())
						}
					}
				});
				drop(this);
				f.await.ok();
			};

			Arc::try_unwrap(r)
				.map_err(|_| ())
				.expect("The `Arc`'s clone is dropped in the previous line.")
				.into_inner()
				.expect("unreachable")
				.expect("unreachable")
		}
	}

	type ChangeEager<'f> = private::DetachedFuture<'f, Result<Result<T, T>, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	fn replace_eager<'f>(&self, new_value: T) -> private::DetachedFuture<'f, Result<T, T>>
	where
		Self: 'f + Sized,
		T: Sized,
	{
		let this = self.downgrade();
		async move {
			let r = Arc::new(Mutex::new(Some(Err(new_value))));
			if let Some(this) = this.upgrade() {
				let f = this.update_async({
					let r = Arc::downgrade(&r);
					move |value| {
						let Some(r) = r.upgrade() else {
							return (Propagation::Halt, ());
						};
						let mut r = r.try_lock().unwrap();
						let new_value = r.take().unwrap().map(|_| ()).unwrap_err();
						*r = Some(Ok(mem::replace(value, new_value)));
						(Propagation::Propagate, ())
					}
				});
				drop(this);
				f.await.ok();
			};

			Arc::try_unwrap(r)
				.map_err(|_| ())
				.expect("The `Arc`'s clone is dropped in the previous line.")
				.into_inner()
				.expect("unreachable")
				.expect("unreachable")
		}
	}

	type ReplaceEager<'f> = private::DetachedFuture<'f, Result<T, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	//TODO: Turn this into an eager method too.
	fn update_eager<'f, U: Send, F: 'f + Send + FnOnce(&mut T) -> (Propagation, U)>(
		&self,
		update: F,
	) -> private::DetachedFuture<'f, Result<U, F>>
	where
		Self: 'f + Sized,
	{
		let this = self.downgrade();
		async move {
			let r = Arc::new(Mutex::new(Some(Err(update))));
			if let Some(this) = this.upgrade() {
				let f = this.source_cell.as_ref().update_eager({
					let r = Arc::downgrade(&r);
					move |value| {
						let Some(r) = r.upgrade() else {
							return (Propagation::Halt, ());
						};
						let mut r = r.try_lock().unwrap();
						let update = r.take().unwrap().map(|_| ()).unwrap_err();
						let (propagation, u) = update(value);
						*r = Some(Ok(u));
						(propagation, ())
					}
				});
				drop(this);
				f.await.ok();
			};

			Arc::try_unwrap(r)
				.map_err(|_| ())
				.expect("The `Arc`'s clone is dropped in the previous line.")
				.into_inner()
				.expect("unreachable")
				.expect("unreachable")
		}
	}

	type UpdateEager<'f, U: 'f, F: 'f> = private::DetachedFuture<'f, Result<U, F>>
	where
		Self: 'f + Sized;

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
}

#[repr(transparent)]
struct Private<T>(T);

impl<T: Send + Sized + ?Sized, SR: ?Sized + SignalRuntimeRef> SourcePin<SR>
	for Private<SignalCellSR<T, dyn SourceCell<T, SR>, SR>>
where
	<SR as SignalRuntimeRef>::Symbol: Sync,
{
	type Output = T;

	fn touch(&self) {
		self.0.touch()
	}

	fn get_clone(&self) -> Self::Output
	where
		Self::Output: Sync + Clone,
	{
		self.0.get_clone()
	}

	fn get_clone_exclusive(&self) -> Self::Output
	where
		Self::Output: Clone,
	{
		self.0.get_clone_exclusive()
	}

	fn read<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Output>>
	where
		Self::Output: 'r + Sync,
	{
		self.0.read()
	}

	fn read_exclusive<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Output>> {
		self.0.read_exclusive()
	}

	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		self.0.clone_runtime_ref()
	}
}

/// This unfortunately must be private/via-`dyn`-only until the non-dispatchable items can be implemented.
impl<T: Send + Sized + ?Sized, SR: ?Sized + SignalRuntimeRef> SourceCellPin<T, SR>
	for Private<SignalCellSR<T, dyn SourceCell<T, SR>, SR>>
where
	<SR as SignalRuntimeRef>::Symbol: Sync,
{
	fn change(&self, new_value: T)
	where
		T: 'static + Sized + PartialEq,
	{
		self.0.source_cell.as_ref().change(new_value)
	}

	fn replace(&self, new_value: T)
	where
		T: 'static + Sized,
	{
		self.0.source_cell.as_ref().replace(new_value)
	}

	fn update(&self, update: impl 'static + Send + FnOnce(&mut T) -> Propagation)
	where
		Self: Sized,
		<SR as SignalRuntimeRef>::Symbol: Sync,
	{
		unreachable!()
	}

	fn change_async<'f>(
		&self,
		new_value: T,
	) -> impl 'f + Send + Future<Output = Result<Result<T, T>, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized + PartialEq,
	{
		unreachable!();
		async { unreachable!() }
	}

	fn replace_async<'f>(&self, new_value: T) -> impl 'f + Send + Future<Output = Result<T, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized,
	{
		unreachable!();
		async { unreachable!() }
	}

	fn update_async<'f, U: Send, F: 'f + Send + FnOnce(&mut T) -> (Propagation, U)>(
		&self,
		update: F,
	) -> impl 'f + Send + Future<Output = Result<U, F>>
	where
		Self: 'f + Sized,
	{
		unreachable!();
		async { unreachable!() }
	}

	fn change_blocking(&self, new_value: T) -> Result<T, T>
	where
		T: Sized + PartialEq,
	{
		self.0.source_cell.as_ref().change_blocking(new_value)
	}

	fn replace_blocking(&self, new_value: T) -> T
	where
		T: Sized,
	{
		self.0.source_cell.as_ref().replace_blocking(new_value)
	}

	fn update_blocking<U>(&self, update: impl FnOnce(&mut T) -> (Propagation, U)) -> U
	where
		Self: Sized,
	{
		unreachable!()
	}
}

impl<'a: 'static, T: 'a + Send + Sized + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> Deref
	for ErasedSignalCell<'a, T, SR>
where
	<SR as SignalRuntimeRef>::Symbol: Sync,
{
	type Target = dyn 'a + SourceCellPin<T, SR>;

	fn deref(&self) -> &Self::Target {
		unsafe {
			&*(mem::transmute::<*const Self, &'a Private<Self>>(self as *const _)
				as *const (dyn 'a + SourceCellPin<T, SR>))
		}
	}
}

/// Duplicated to avoid identities.
mod private {
	use std::{
		future::Future,
		pin::Pin,
		task::{Context, Poll},
	};

	use futures_lite::FutureExt;

	pub struct DetachedFuture<'f, Output: 'f>(
		pub(super) Pin<Box<dyn 'f + Send + Future<Output = Output>>>,
	);

	impl<'f, Output: 'f> Future for DetachedFuture<'f, Output> {
		type Output = Output;

		fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
			self.0.poll(cx)
		}
	}
}
