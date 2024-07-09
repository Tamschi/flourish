use std::{borrow::Borrow, fmt::Debug, marker::PhantomData, pin::Pin, sync::Arc};

use isoprenoid::runtime::{CallbackTableTypes, GlobalSignalRuntime, SignalRuntimeRef, Update};

use crate::{
	raw::{InertCell, ReactiveCell},
	traits::{Source, SourceCell, Subscribable},
	SignalRef, SignalSR, SourceCellPin, SourcePin,
};

/// Type inference helper alias for [`SignalCellSR`] (using [`GlobalSignalRuntime`]).
pub type SignalCell<T, S> = SignalCellSR<T, S, GlobalSignalRuntime>;

//TODO: `WeakSignalCell`.

#[derive(Clone)]
pub struct SignalCellSR<
	T: ?Sized + Send,
	S: ?Sized + SourceCell<T, SR>,
	SR: ?Sized + SignalRuntimeRef<Symbol: Sync>,
> {
	source_cell: Pin<Arc<S>>,
	_phantom: PhantomData<AssertSync<(PhantomData<T>, SR)>>,
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

struct AssertSync<T: ?Sized>(T);
unsafe impl<T: ?Sized> Sync for AssertSync<T> {}

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
		Self {
			source_cell: Arc::pin(InertCell::with_runtime(initial_value, runtime)),
			_phantom: PhantomData,
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

impl<
		T: Send,
		HandlerFnPin: Send + FnMut(<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
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
		Self {
			source_cell: Arc::pin(ReactiveCell::with_runtime(
				initial_value,
				on_subscribed_change_fn_pin,
				runtime,
			)),
			_phantom: PhantomData,
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
	/// Cheaply borrows this [`SignalCell`] as [`SignalRef`], which is [`Copy`].
	pub fn as_ref<'a>(&self) -> SignalRef<'_, 'a, T, SR>
	where
		T: 'a,
		SR: 'a,
	{
		SignalRef {
			source: {
				let ptr = Arc::into_raw(unsafe {
					Pin::into_inner_unchecked(Pin::clone(&self.source_cell))
				});
				unsafe { Arc::decrement_strong_count(ptr) };
				ptr
			},
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

	//TODO: "Splitting".
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

	fn update(&self, update: impl 'static + Send + FnOnce(&mut T) -> Update)
	where
		Self: Sized,
		<SR as SignalRuntimeRef>::Symbol: Sync,
	{
		self.source_cell.as_ref().update(update)
	}

	fn update_async<U: Send>(
		&self,
		update: impl Send + FnOnce(&mut T) -> (U, Update),
	) -> impl Send + std::future::Future<Output = U>
	where
		Self: Sized,
	{
		self.source_cell.as_ref().update_async(update)
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

	fn update_blocking<U>(&self, update: impl FnOnce(&mut T) -> (U, Update)) -> U
	where
		Self: Sized,
	{
		self.source_cell.as_ref().update_blocking(update)
	}
}

//TODO: Are the non-dispatchable methods callable on this?
//      Otherwise, allow use of the trait *only* through the trait object.
impl<T: Send + Sized + ?Sized, SR: ?Sized + SignalRuntimeRef> SourceCellPin<T, SR>
	for SignalCellSR<T, dyn SourceCell<T, SR>, SR>
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

	fn update(&self, update: impl 'static + Send + FnOnce(&mut T) -> Update)
	where
		Self: Sized,
		<SR as SignalRuntimeRef>::Symbol: Sync,
	{
		unimplemented!()
	}

	fn update_async<U: Send>(
		&self,
		update: impl Send + FnOnce(&mut T) -> (U, Update),
	) -> impl Send + std::future::Future<Output = U>
	where
		Self: Sized,
	{
		unimplemented!();
		async { unimplemented!() }
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

	fn update_blocking<U>(&self, update: impl FnOnce(&mut T) -> (U, Update)) -> U
	where
		Self: Sized,
	{
		unimplemented!()
	}
}
