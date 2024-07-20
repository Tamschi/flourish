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

	fn change_async<'f>(
		self: Pin<&Self>,
		new_value: T,
	) -> private::DetachedAsyncFuture<'f, Result<Result<T, T>, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized + PartialEq,
	{
		let this = self.downgrade();
		private::DetachedAsyncFuture(Box::pin(async move {
			if let Some(this) = this.upgrade() {
				this.change_eager(new_value).await
			} else {
				Err(new_value)
			}
		}))
	}

	type ChangeAsync<'f> = private::DetachedAsyncFuture<'f, Result<Result<T,T>, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	fn replace_async<'f>(
		self: Pin<&Self>,
		new_value: T,
	) -> private::DetachedAsyncFuture<'f, Result<T, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized,
	{
		let this = self.downgrade();
		private::DetachedAsyncFuture(Box::pin(async move {
			if let Some(this) = this.upgrade() {
				this.replace_eager(new_value).await
			} else {
				Err(new_value)
			}
		}))
	}

	type ReplaceAsync<'f> = private::DetachedAsyncFuture<'f, Result<T, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	fn update_async<'f, U: 'f + Send, F: 'f + Send + FnOnce(&mut T) -> (Propagation, U)>(
		self: Pin<&Self>,
		update: F,
	) -> private::DetachedAsyncFuture<'f, Result<U, F>>
	where
		Self: 'f + Sized,
	{
		let this = self.downgrade();
		private::DetachedAsyncFuture(Box::pin(async move {
			if let Some(this) = this.upgrade() {
				this.update_eager(update).await
			} else {
				Err(update)
			}
		}))
	}

	type UpdateAsync<'f, U: 'f, F: 'f>=private::DetachedAsyncFuture<'f, Result<U, F>>
	where
		Self: 'f + Sized;

	fn change_eager<'f>(
		&self,
		new_value: T,
	) -> private::DetachedEagerFuture<'f, Result<Result<T, T>, T>>
	where
		Self: 'f + Sized,
		T: Sized + PartialEq,
	{
		private::DetachedEagerFuture(Box::pin(self.source_cell.as_ref().change_eager(new_value)))
	}

	type ChangeEager<'f> = private::DetachedEagerFuture<'f, Result<Result<T, T>, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	fn replace_eager<'f>(&self, new_value: T) -> private::DetachedEagerFuture<'f, Result<T, T>>
	where
		Self: 'f + Sized,
		T: Sized,
	{
		private::DetachedEagerFuture(Box::pin(self.source_cell.as_ref().replace_eager(new_value)))
	}

	type ReplaceEager<'f> = private::DetachedEagerFuture<'f, Result<T, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	fn update_eager<'f, U: Send, F: 'f + Send + FnOnce(&mut T) -> (Propagation, U)>(
		&self,
		update: F,
	) -> private::DetachedEagerFuture<'f, Result<U, F>>
	where
		Self: 'f + Sized,
	{
		private::DetachedEagerFuture(Box::pin(self.source_cell.as_ref().update_eager(update)))
	}

	type UpdateEager<'f, U: 'f, F: 'f> = private::DetachedEagerFuture<'f, Result<U, F>>
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

/// ⚠️ This implementation uses dynamic dispatch internally for all methods with `Self: Sized`
/// bound, which is a bit less performant than using those accessors without type erasure.
impl<'a, T: Send + Sized + ?Sized, SR: ?Sized + SignalRuntimeRef> SourceCellPin<T, SR>
	for ErasedSignalCell<'a, T, SR>
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
		todo!("dyncall")
	}

	fn change_async<'f>(
		self: Pin<&Self>,
		new_value: T,
	) -> private::DetachedAsyncFuture<'f, Result<Result<T, T>, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized + PartialEq,
	{
		let this = self.downgrade();
		private::DetachedAsyncFuture(Box::pin(async move {
			if let Some(this) = this.upgrade() {
				this.change_eager(new_value).await
			} else {
				Err(new_value)
			}
		}))
	}

	type ChangeAsync<'f> = private::DetachedAsyncFuture<'f, Result<Result<T,T>, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	fn replace_async<'f>(
		self: Pin<&Self>,
		new_value: T,
	) -> private::DetachedAsyncFuture<'f, Result<T, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized,
	{
		let this = self.downgrade();
		private::DetachedAsyncFuture(Box::pin(async move {
			if let Some(this) = this.upgrade() {
				this.replace_eager(new_value).await
			} else {
				Err(new_value)
			}
		}))
	}

	type ReplaceAsync<'f> = private::DetachedAsyncFuture<'f, Result<T, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	fn update_async<'f, U: 'f + Send, F: 'f + Send + FnOnce(&mut T) -> (Propagation, U)>(
		self: Pin<&Self>,
		update: F,
	) -> private::DetachedAsyncFuture<'f, Result<U, F>>
	where
		Self: 'f + Sized,
	{
		let this = self.downgrade();
		private::DetachedAsyncFuture(Box::pin(async move {
			if let Some(this) = this.upgrade() {
				this.update_eager(update).await
			} else {
				Err(update)
			}
		}))
	}

	type UpdateAsync<'f, U: 'f, F: 'f>=private::DetachedAsyncFuture<'f, Result<U, F>>
	where
		Self: 'f + Sized;

	fn change_eager<'f>(
		&self,
		new_value: T,
	) -> private::DetachedEagerFuture<'f, Result<Result<T, T>, T>>
	where
		Self: 'f + Sized,
		T: Sized + PartialEq,
	{
		todo!("dyncall")
	}

	type ChangeEager<'f> = private::DetachedEagerFuture<'f, Result<Result<T, T>, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	fn replace_eager<'f>(&self, new_value: T) -> private::DetachedEagerFuture<'f, Result<T, T>>
	where
		Self: 'f + Sized,
		T: Sized,
	{
		todo!("dyncall")
	}

	type ReplaceEager<'f> = private::DetachedEagerFuture<'f, Result<T, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	fn update_eager<'f, U: Send, F: 'f + Send + FnOnce(&mut T) -> (Propagation, U)>(
		&self,
		update: F,
	) -> private::DetachedEagerFuture<'f, Result<U, F>>
	where
		Self: 'f + Sized,
	{
		todo!("dyncall")
	}

	type UpdateEager<'f, U: 'f, F: 'f> = private::DetachedEagerFuture<'f, Result<U, F>>
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
		todo!("dyncall")
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

	#[must_use = "Eager futures may still cancel their effect iff dropped."]
	pub struct DetachedEagerFuture<'f, Output: 'f>(
		pub(super) Pin<Box<dyn 'f + Send + Future<Output = Output>>>,
	);

	impl<'f, Output: 'f> Future for DetachedEagerFuture<'f, Output> {
		type Output = Output;

		fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
			self.0.poll(cx)
		}
	}

	#[must_use = "Async futures do nothing unless awaited."]
	pub struct DetachedAsyncFuture<'f, Output: 'f>(
		pub(super) Pin<Box<dyn 'f + Send + Future<Output = Output>>>,
	);

	impl<'f, Output: 'f> Future for DetachedAsyncFuture<'f, Output> {
		type Output = Output;

		fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
			self.0.poll(cx)
		}
	}
}
