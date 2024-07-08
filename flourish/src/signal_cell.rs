use std::{borrow::Borrow, fmt::Debug, pin::Pin, sync::Arc};

use isoprenoid::runtime::{GlobalSignalRuntime, SignalRuntimeRef, Update};

use crate::{
	raw::SourceCell,
	traits::{Source, Subscribable},
	SignalSR, SourcePin,
};

/// Type inference helper alias for [`SignalCellSR`] (using [`GlobalSignalRuntime`]).
pub type SignalCell<T> = SignalCellSR<T, GlobalSignalRuntime>;

#[derive(Clone)]
pub struct SignalCellSR<T: ?Sized + Send, SR: SignalRuntimeRef> {
	source_cell: Pin<Arc<SourceCell<T, SR>>>,
}

impl<T: ?Sized + Debug + Send, SR: SignalRuntimeRef + Debug> Debug for SignalCellSR<T, SR>
where
	SR::Symbol: Debug,
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_tuple("SignalCell")
			.field(&self.source_cell)
			.finish()
	}
}

impl<T: Send, SR: SignalRuntimeRef> SignalCellSR<T, SR> {
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
			source_cell: Arc::pin(SourceCell::with_runtime(initial_value, runtime)),
		}
	}

	/// Cheaply creates a [`SignalSR`] handle to the managed [`SourceCell`].
	pub fn to_signal<'a>(&self) -> SignalSR<'a, T, SR>
	where
		T: 'a,
		SR: 'a,
	{
		SignalSR {
			source: Pin::clone(&self.source_cell) as Pin<Arc<dyn Subscribable<SR, Output = T>>>,
		}
	}

	pub fn read<'r>(&'r self) -> impl 'r + Borrow<T>
	where
		T: Sync,
	{
		self.source_cell.read()
	}

	pub fn read_exclusive<'r>(&'r self) -> impl 'r + Borrow<T> {
		self.source_cell.read_exclusive()
	}

	pub fn change(&self, new_value: T)
	where
		T: 'static + Send + PartialEq,
		SR: Sync,
		SR::Symbol: Sync,
	{
		self.source_cell.as_ref().change(new_value)
	}

	pub fn replace(&self, new_value: T)
	where
		T: 'static + Send,
		SR: Sync,
		SR::Symbol: Sync,
	{
		self.source_cell.as_ref().replace(new_value)
	}

	pub fn update(&self, update: impl 'static + Send + FnOnce(&mut T) -> Update)
	where
		SR: Sync,
		SR::Symbol: Sync,
	{
		self.source_cell.as_ref().update(update)
	}

	pub async fn change_async(&self, new_value: T) -> Result<T, T>
	where
		T: Send + PartialEq,
		SR: Sync,
		SR::Symbol: Sync,
	{
		self.source_cell.as_ref().change_async(new_value).await
	}

	pub async fn replace_async(&self, new_value: T) -> T
	where
		T: Send,
		SR: Sync,
		SR::Symbol: Sync,
	{
		self.source_cell.as_ref().replace_async(new_value).await
	}

	pub async fn update_async<U: Send>(
		&self,
		update: impl Send + FnOnce(&mut T) -> (U, Update),
	) -> U
	where
		SR: Sync,
		SR::Symbol: Sync,
	{
		self.source_cell.as_ref().update_async(update).await
	}

	pub fn change_blocking(&self, new_value: T) -> Result<T, T>
	where
		T: PartialEq,
	{
		self.source_cell.change_blocking(new_value)
	}

	pub fn replace_blocking(&self, new_value: T) -> T {
		self.source_cell.replace_blocking(new_value)
	}

	pub fn update_blocking<U>(&self, update: impl FnOnce(&mut T) -> (U, Update)) -> U {
		self.source_cell.update_blocking(update)
	}

	pub fn into_signal_and_setter<'a, S>(
		self,
		into_setter: impl FnOnce(Self) -> S,
	) -> (SignalSR<'a, T, SR>, S)
	where
		T: 'a + Sized,
		SR: 'a,
	{
		(self.to_signal(), into_setter(self))
	}

	pub fn into_getter_and_setter<'a, S, R>(
		self,
		signal_into_getter: impl FnOnce(SignalSR<'a, T, SR>) -> R,
		into_setter: impl FnOnce(Self) -> S,
	) -> (R, S)
	where
		T: 'a + Sized,
		SR: 'a,
	{
		(signal_into_getter(self.to_signal()), into_setter(self))
	}
}

impl<T: Send + Sized + ?Sized, SR: ?Sized + SignalRuntimeRef> SourcePin<SR>
	for SignalCellSR<T, SR>
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
		self.source_cell.as_ref().read()
	}

	fn read_exclusive<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Output>> {
		self.source_cell.as_ref().read_exclusive()
	}

	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		self.source_cell.as_ref().clone_runtime_ref()
	}
}
