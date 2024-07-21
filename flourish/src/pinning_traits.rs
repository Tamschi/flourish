use std::{
	borrow::Borrow,
	future::Future,
	mem,
	pin::Pin,
	sync::{Arc, Mutex},
};

use futures_lite::FutureExt;
use isoprenoid::runtime::{Propagation, SignalsRuntimeRef};

pub unsafe trait EnableValuePinAPI {}

use crate::{
	traits::{Source, SourceCell, Subscribable},
	SourceCellPin, SourcePin,
};

/// **Combinators should implement this.** Interface for "raw" (stack-pinnable) signals that have an accessible value.
///
/// # Safety Notes
///
/// It's sound to transmute [`dyn Source<_>`](`Source`) between different associated `Value`s as long as that's sound and they're ABI-compatible.
///
/// Note that dropping the [`dyn Source<_>`](`Source`) dynamically **transmutes back**.
pub trait PinningSource<SR: ?Sized + SignalsRuntimeRef>: Send + Sync + Sealed {
	/// The type of value presented by the [`Source`].
	type Output: ?Sized + Send;

	/// Records `self` as dependency without accessing the value.
	fn touch(self: Pin<&Self>);

	/// Records `self` as dependency and retrieves a copy of the value.
	///
	/// Prefer [`Source::touch`] where possible.
	fn get(self: Pin<&Self>) -> Self::Output
	where
		Self::Output: Sync + Copy,
	{
		self.get_clone()
	}

	/// Records `self` as dependency and retrieves a clone of the value.
	///
	/// Prefer [`Source::get`] where available.
	fn get_clone(self: Pin<&Self>) -> Self::Output
	where
		Self::Output: Sync + Clone;

	/// Records `self` as dependency and retrieves a copy of the value.
	///
	/// Prefer [`Source::get`] where available.
	fn get_exclusive(self: Pin<&Self>) -> Self::Output
	where
		Self::Output: Copy,
	{
		self.get_clone_exclusive()
	}

	/// Records `self` as dependency and retrieves a clone of the value.
	///
	/// Prefer [`Source::get_clone`] where available.
	fn get_clone_exclusive(self: Pin<&Self>) -> Self::Output
	where
		Self::Output: Clone;

	/// Records `self` as dependency and allows borrowing the value.
	///
	/// Prefer a type-associated `.read()` method where available.
	fn read<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + PinningBorrow<Self::Output>>
	where
		Self::Output: Sync;

	/// Records `self` as dependency and allows borrowing the value.
	///
	/// Prefer a type-associated `.read()` method where available.  
	/// Otherwise, prefer a type-associated `.read_exclusive()` method where available.  
	/// Otherwise, prefer [`Source::read`] where available.
	fn read_exclusive<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + PinningBorrow<Self::Output>>;

	/// Clones this [`SourcePin`]'s [`SignalsRuntimeRef`].
	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized;
}

/// **Most application code should consume this.** Interface for movable signal handles that have an accessible value.
pub trait PinningSourcePin<SR: ?Sized + SignalsRuntimeRef>: Send + Sync + Sealed {
	/// The type of value presented by the [`SourcePin`].
	type Output: ?Sized + Send;

	/// Records `self` as dependency without accessing the value.
	fn touch(&self);

	/// Records `self` as dependency and retrieves a copy of the value.
	///
	/// Prefer [`SourcePin::touch`] where possible.
	fn get(&self) -> Self::Output
	where
		Self::Output: Sync + Copy,
	{
		self.get_clone()
	}

	/// Records `self` as dependency and retrieves a clone of the value.
	///
	/// Prefer [`SourcePin::get`] where available.
	fn get_clone(&self) -> Self::Output
	where
		Self::Output: Sync + Clone;

	/// Records `self` as dependency and retrieves a copy of the value.
	///
	/// Prefer [`SourcePin::get`] where available.
	fn get_exclusive(&self) -> Self::Output
	where
		Self::Output: Copy,
	{
		self.get_clone_exclusive()
	}

	/// Records `self` as dependency and retrieves a clone of the value.
	///
	/// Prefer [`SourcePin::get_clone`] where available.
	fn get_clone_exclusive(&self) -> Self::Output
	where
		Self::Output: Clone;

	/// Records `self` as dependency and allows borrowing the value.
	///
	/// Prefer a type-associated `.read()` method where available.
	fn read<'r>(&'r self) -> Box<dyn 'r + PinningBorrow<Self::Output>>
	where
		Self::Output: 'r + Sync;

	/// Records `self` as dependency and allows borrowing the value.
	///
	/// Prefer a type-associated `.read()` method where available.  
	/// Otherwise, prefer a type-associated `.read_exclusive()` method where available.  
	/// Otherwise, prefer [`SourcePin::read`] where available.
	fn read_exclusive<'r>(&'r self) -> Box<dyn 'r + PinningBorrow<Self::Output>>;

	/// Clones this [`SourcePin`]'s [`SignalsRuntimeRef`].
	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized;
}

/// [`Cell`](`core::cell::Cell`)-likes that announce changes to their values to a [`SignalsRuntimeRef`].
///
/// The "update" and "async" methods are non-dispatchable (meaning they can't be called on trait objects).
pub trait PinningSourceCell<T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef>:
	Send + Sync + Subscribable<SR, Output = T> + Sealed
{
	/// Iff `new_value` differs from the current value, replaces it and signals dependents.
	///
	/// # Logic
	///
	/// This method **must not** block *indefinitely*.  
	/// This method **may** defer its effect.
	fn change(self: Pin<&Self>, new_value: T)
	where
		T: 'static + Sized + PartialEq;

	/// Unconditionally replaces the current value with `new_value` and signals dependents.
	///
	/// Prefer [`.change(new_value)`] if debouncing is acceptable.
	///
	/// # Logic
	///
	/// This method **must not** block *indefinitely*.  
	/// This method **may** defer its effect.
	fn replace(self: Pin<&Self>, new_value: T)
	where
		T: 'static + Sized;

	/// Modifies the current value using the given closure.
	///
	/// The closure decides whether to signal dependents.
	///
	/// # Logic
	///
	/// This method **must not** block *indefinitely*.  
	/// This method **may** defer its effect.
	fn update(self: Pin<&Self>, update: impl 'static + Send + FnOnce(&mut T) -> Propagation)
	where
		Self: Sized,
		T: 'static;

	/// The same as [`update`](`SourceCell::update`), but object-safe.
	fn update_dyn(
		self: Pin<&Self>,
		update: Box<dyn 'static + Send + FnOnce(&mut T) -> Propagation>,
	) where
		T: 'static;

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
	/// This method's effect **should** be cancelled iff the returned [`Future`] is dropped before it would yield [`Ready`](`core::task::Poll::Ready`).  
	/// The returned [`Future`] **may** return [`Pending`](`core::task::Poll::Pending`) indefinitely iff polled in signal callbacks.
	///
	/// Don't `.await` the returned [`Future`] in signal callbacks!
	fn change_eager<'f>(self: Pin<&Self>, new_value: T) -> Self::ChangeEager<'f>
	where
		Self: 'f + Sized,
		T: 'f + Sized + PartialEq;

	type ChangeEager<'f>: 'f + Send + Future<Output = Result<Result<T, T>, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

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
	/// This method's effect **should** be cancelled iff the returned [`Future`] is dropped before it would yield [`Ready`](`core::task::Poll::Ready`).  
	/// The returned [`Future`] **may** return [`Pending`](`core::task::Poll::Pending`) indefinitely iff polled in signal callbacks.
	///
	/// Don't `.await` the returned [`Future`] in signal callbacks!
	fn replace_eager<'f>(self: Pin<&Self>, new_value: T) -> Self::ReplaceEager<'f>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	type ReplaceEager<'f>: 'f + Send + Future<Output = Result<T, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

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
	/// This method **should** apply its effect even if [`Future`] is not polled.  
	/// The returned [`Future`] **may** return [`Pending`](`core::task::Poll::Pending`) indefinitely iff polled in signal callbacks.
	///
	/// Don't `.await` the returned [`Future`] in signal callbacks!
	fn update_eager<'f, U: 'f + Send, F: 'f + Send + FnOnce(&mut T) -> (Propagation, U)>(
		self: Pin<&Self>,
		update: F,
	) -> Self::UpdateEager<'f, U, F>
	where
		Self: 'f + Sized;

	type UpdateEager<'f, U: 'f, F: 'f>: 'f + Send + Future<Output = Result<U, F>>
	where
		Self: 'f + Sized;

	/// The same as [`change_eager`](`SourceCell::change_eager`), but object-safe.
	fn change_eager_dyn<'f>(
		self: Pin<&Self>,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<Result<T, T>, T>>>
	where
		T: 'f + Sized + PartialEq;

	/// The same as [`replace_eager`](`SourceCell::replace_eager`), but object-safe.
	fn replace_eager_dyn<'f>(
		self: Pin<&Self>,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<T, T>>>
	where
		T: 'f + Sized;

	/// The same as [`update_eager`](`SourceCell::update_eager`), but object-safe.
	fn update_eager_dyn<'f>(
		self: Pin<&Self>,
		update: Box<dyn 'f + Send + FnOnce(&mut T) -> Propagation>,
	) -> Box<
		dyn 'f
			+ Send
			+ Future<Output = Result<(), Box<dyn 'f + Send + FnOnce(&mut T) -> Propagation>>>,
	>
	where
		T: 'f;

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
	fn change_blocking(&self, new_value: T) -> Result<T, T>
	where
		T: Sized + PartialEq;

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
	fn replace_blocking(&self, new_value: T) -> T
	where
		T: Sized;

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
	fn update_blocking<U>(&self, update: impl FnOnce(&mut T) -> (Propagation, U)) -> U
	where
		Self: Sized;

	/// The same as [`update_blocking`](`SourceCell::update_blocking`), but object-safe.
	fn update_blocking_dyn(&self, update: Box<dyn '_ + FnOnce(&mut T) -> Propagation>);

	fn as_source_and_cell(
		self: Pin<&Self>,
	) -> (
		Pin<&impl PinningSource<SR, Output = T>>,
		Pin<&impl PinningSourceCell<T, SR>>,
	)
	where
		Self: Sized,
	{
		(self, self)
	}
}

/// [`Cell`](`core::cell::Cell`)-likes that announce changes to their values to a [`SignalsRuntimeRef`].
///
/// The "update" and "async" methods are non-dispatchable (meaning they can't be called on trait objects).
pub trait PinningSourceCellPin<T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef>:
	Send + Sync + PinningSourcePin<SR, Output = T> + Sealed
{
	/// Iff `new_value` differs from the current value, replaces it and signals dependents.
	///
	/// # Logic
	///
	/// This method **must not** block *indefinitely*.  
	/// This method **may** defer its effect.
	fn change(&self, new_value: T)
	where
		T: 'static + Sized + PartialEq;

	/// Unconditionally replaces the current value with `new_value` and signals dependents.
	///
	/// Prefer [`.change(new_value)`] if debouncing is acceptable.
	///
	/// # Logic
	///
	/// This method **must not** block *indefinitely*.  
	/// This method **may** defer its effect.
	fn replace(&self, new_value: T)
	where
		T: 'static + Sized;

	/// Modifies the current value using the given closure.
	///
	/// The closure decides whether to signal dependents.
	///
	/// # Logic
	///
	/// This method **must not** block *indefinitely*.  
	/// This method **may** defer its effect.
	fn update(&self, update: impl 'static + Send + FnOnce(&mut T) -> Propagation)
	where
		Self: Sized,
		T: 'static;

	/// The same as [`update`](`SourceCellPin::update`), but object-safe.
	fn update_dyn(&self, update: Box<dyn 'static + Send + FnOnce(&mut T) -> Propagation>)
	where
		T: 'static;

	/// Cheaply creates a [`Future`] that has the effect of [`change_eager`](`SourceCellPin::change_eager`) when polled.
	///
	/// # Logic
	///
	/// The [`Future`] **should not** hold a strong reference to `self`.
	fn change_async<'f>(&self, new_value: T) -> Self::ChangeAsync<'f>
	where
		Self: 'f + Sized,
		T: 'f + Sized + PartialEq;

	type ChangeAsync<'f>: 'f + Send + Future<Output = Result<Result<T, T>, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	/// Cheaply creates a [`Future`] that has the effect of [`replace_eager`](`SourceCellPin::replace_eager`) when polled.
	///
	/// # Logic
	///
	/// The [`Future`] **should not** hold a strong reference to `self`.
	fn replace_async<'f>(&self, new_value: T) -> Self::ReplaceAsync<'f>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	type ReplaceAsync<'f>: 'f + Send + Future<Output = Result<T, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	/// Cheaply creates a [`Future`] that has the effect of [`update_eager`](`SourceCellPin::update_eager`) when polled.
	///
	/// # Logic
	///
	/// The [`Future`] **should not** hold a strong reference to `self`.
	fn update_async<'f, U: 'f + Send, F: 'f + Send + FnOnce(&mut T) -> (Propagation, U)>(
		&self,
		update: F,
	) -> Self::UpdateAsync<'f, U, F>
	where
		Self: 'f + Sized;

	type UpdateAsync<'f, U: 'f, F: 'f>: 'f + Send + Future<Output = Result<U, F>>
	where
		Self: 'f + Sized;

	/// The same as [`change_async`](`SourceCellPin::change_async`), but object-safe.
	fn change_async_dyn<'f>(
		&self,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<Result<T, T>, T>>>
	where
		T: 'f + Sized + PartialEq;

	/// The same as [`replace_async`](`SourceCellPin::replace_async`), but object-safe.
	fn replace_async_dyn<'f>(
		&self,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<T, T>>>
	where
		T: 'f + Sized;

	/// The same as [`update_async`](`SourceCellPin::update_async`), but object-safe.
	fn update_async_dyn<'f>(
		&self,
		update: Box<dyn 'f + Send + FnOnce(&mut T) -> Propagation>,
	) -> Box<
		dyn 'f
			+ Send
			+ Future<Output = Result<(), Box<dyn 'f + Send + FnOnce(&mut T) -> Propagation>>>,
	>
	where
		T: 'f;

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
	fn change_eager<'f>(&self, new_value: T) -> Self::ChangeEager<'f>
	where
		Self: 'f + Sized,
		T: 'f + Sized + PartialEq;

	type ChangeEager<'f>: 'f + Send + Future<Output = Result<Result<T, T>, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

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
	fn replace_eager<'f>(&self, new_value: T) -> Self::ReplaceEager<'f>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	type ReplaceEager<'f>: 'f + Send + Future<Output = Result<T, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

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
	fn update_eager<'f, U: Send, F: 'f + Send + FnOnce(&mut T) -> (Propagation, U)>(
		&self,
		update: F,
	) -> Self::UpdateEager<'f, U, F>
	where
		Self: 'f + Sized;

	type UpdateEager<'f, U: 'f, F: 'f>: 'f + Send + Future<Output = Result<U, F>>
	where
		Self: 'f + Sized;

	/// The same as [`change_eager`](`SourceCellPin::change_eager`), but object-safe.
	fn change_eager_dyn<'f>(
		&self,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<Result<T, T>, T>>>
	where
		T: 'f + Sized + PartialEq;

	/// The same as [`replace_eager`](`SourceCellPin::replace_eager`), but object-safe.
	fn replace_eager_dyn<'f>(
		&self,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<T, T>>>
	where
		T: 'f + Sized;

	/// The same as [`update_eager`](`SourceCellPin::update_eager`), but object-safe.
	fn update_eager_dyn<'f>(
		&self,
		update: Box<dyn 'f + Send + FnOnce(&mut T) -> Propagation>,
	) -> Box<
		dyn 'f
			+ Send
			+ Future<Output = Result<(), Box<dyn 'f + Send + FnOnce(&mut T) -> Propagation>>>,
	>
	where
		T: 'f;

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
	fn change_blocking(&self, new_value: T) -> Result<T, T>
	where
		T: Sized + PartialEq;

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
	fn replace_blocking(&self, new_value: T) -> T
	where
		T: Sized;

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
	fn update_blocking<U>(&self, update: impl FnOnce(&mut T) -> (Propagation, U)) -> U
	where
		Self: Sized;

	/// The same as [`update_blocking`](`SourceCellPin::update_blocking`), but object-safe.
	fn update_blocking_dyn(&self, update: Box<dyn '_ + FnOnce(&mut T) -> Propagation>);
}

unsafe fn as_inner<T>(pin: &Pin<T>) -> &T {
	unsafe { mem::transmute::<&'_ Pin<T>, &'_ T>(pin) }
}

unsafe fn as_inner_pin<T>(pin_pin: Pin<&Pin<T>>) -> Pin<&T> {
	unsafe { mem::transmute::<Pin<&'_ Pin<T>>, Pin<&'_ T>>(pin_pin) }
}

impl<S: Source<SR>, SR: ?Sized + SignalsRuntimeRef> PinningSource<SR> for Pin<S>
where
	S: EnableValuePinAPI,
{
	type Output = S::Output;

	fn touch(self: Pin<&Self>) {
		unsafe { as_inner_pin(self) }.touch()
	}

	fn get_clone(self: Pin<&Self>) -> Self::Output
	where
		Self::Output: Sync + Clone,
	{
		unsafe { as_inner_pin(self) }.get_clone()
	}

	fn get_clone_exclusive(self: Pin<&Self>) -> Self::Output
	where
		Self::Output: Clone,
	{
		unsafe { as_inner_pin(self) }.get_clone_exclusive()
	}

	fn read<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Borrow<Self::Output>>
	where
		Self::Output: Sync,
	{
		unsafe { as_inner_pin(self) }.read()
	}

	fn read_exclusive<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Borrow<Self::Output>> {
		unsafe { as_inner_pin(self) }.read_exclusive()
	}

	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		unsafe { as_inner(self) }.clone_runtime_ref()
	}
}

impl<S: SourceCell<T, SR>, T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef>
	PinningSourceCell<T, SR> for Pin<S>
where
	S: EnableValuePinAPI,
{
	fn change(self: Pin<&Self>, new_value: T)
	where
		T: 'static + Sized + PartialEq,
	{
		self.update(|mut value| {
			if *value == new_value {
				Propagation::Halt
			} else {
				value.set(new_value);
				Propagation::Propagate
			}
		})
	}

	fn set(self: Pin<&Self>, new_value: T)
	where
		T: 'static + Sized,
	{
		self.update(|mut value| {
			value.set(new_value);
			Propagation::Propagate
		})
	}

	fn update(self: Pin<&Self>, update: impl 'static + Send + FnOnce(Pin<&mut T>) -> Propagation)
	where
		Self: Sized,
	{
		todo!()
	}

	fn update_async<U: Send>(
		self: Pin<&Self>,
		update: impl Send + FnOnce(Pin<&mut T>) -> (Propagation, U),
	) -> impl Send + Future<Output = U>
	where
		Self: Sized,
	{
		todo!();
		async { todo!() }
	}

	fn change_blocking(self: Pin<&Self>, new_value: T) -> Result<(), T>
	where
		T: Sized + PartialEq,
	{
		self.update_blocking(|mut value| {
			if *value != new_value {
				(Propagation::Propagate, Ok(value.set(new_value)))
			} else {
				(Propagation::Halt, Err(new_value))
			}
		})
	}

	fn set_blocking(self: Pin<&Self>, new_value: T) -> ()
	where
		T: Sized,
	{
		self.update_blocking(|mut value| (Propagation::Propagate, value.set(new_value)))
	}

	fn update_blocking<U>(
		self: Pin<&Self>,
		update: impl FnOnce(Pin<&mut T>) -> (Propagation, U),
	) -> U
	where
		Self: Sized,
	{
		unsafe { as_inner_pin(self) }
			.update_blocking(|value| unsafe { update(Pin::new_unchecked(value)) })
	}
}

impl<S: PinningSourcePin<SR>, SR: ?Sized + SignalsRuntimeRef> PinningSourcePin<SR> for Pin<S>
where
	S: EnableValuePinAPI,
{
	type Output = S::Output;

	fn touch(&self) {
		unsafe { as_inner(self) }.touch()
	}

	fn get(&self) -> Self::Output
	where
		Self::Output: Sync + Copy,
	{
		unsafe { as_inner(self) }.get()
	}

	fn get_exclusive(&self) -> Self::Output
	where
		Self::Output: Copy,
	{
		unsafe { as_inner(self) }.get_exclusive()
	}

	fn get_clone(&self) -> Self::Output
	where
		Self::Output: Sync + Clone,
	{
		unsafe { as_inner(self) }.get_clone()
	}

	fn get_clone_exclusive(&self) -> Self::Output
	where
		Self::Output: Clone,
	{
		unsafe { as_inner(self) }.get_clone_exclusive()
	}

	fn read<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Output>>
	where
		Self::Output: 'r + Sync,
	{
		unsafe { as_inner(self) }.read()
	}

	fn read_exclusive<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Output>> {
		unsafe { as_inner(self) }.read_exclusive()
	}

	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		unsafe { as_inner(self) }.clone_runtime_ref()
	}
}

/// Note that the '`change`' and '`replace`' methods do not call the non-pinning methods
/// of the same name, as those would move the value. They are implemented through
/// `update` instead.
impl<S: SourceCellPin<T, SR>, T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef>
	PinningSourceCellPin<T, SR> for Pin<S>
where
	S: EnableValuePinAPI,
{
	fn change(&self, new_value: T)
	where
		T: 'static + Sized + PartialEq,
	{
		self.update(|mut value| {
			if *value == new_value {
				Propagation::Halt
			} else {
				value.set(new_value);
				Propagation::Propagate
			}
		})
	}

	fn set(&self, new_value: T)
	where
		T: 'static + Sized,
	{
		self.update(|mut value| {
			value.set(new_value);
			Propagation::Propagate
		})
	}

	fn update(&self, update: impl 'static + Send + FnOnce(Pin<&mut T>) -> Propagation)
	where
		Self: Sized,
		T: 'static,
	{
		unsafe { as_inner(self) }.update(|value| unsafe { update(Pin::new_unchecked(value)) })
	}

	fn update_async<'f, U: 'f + Send, F: 'f + Send + FnOnce(Pin<&mut T>) -> (Propagation, U)>(
		&self,
		update: F,
	) -> impl 'f + Send + Future<Output = Result<U, F>>
	where
		Self: Sized,
		S: 'f,
	{
		let shelf = Arc::new(Mutex::new(Some(Err(update))));
		let f = unsafe { as_inner(self) }.update_async({
			let shelf = Arc::downgrade(&shelf);
			move |value| {
				if let Some(shelf) = shelf.upgrade() {
					let update = shelf
						.try_lock()
						.expect("unreachable")
						.take()
						.expect("unreachable")
						.map(|_| ())
						.expect_err("unreachable");
					let (propagation, u) = unsafe { update(Pin::new_unchecked(value)) };
					assert!(shelf
						.try_lock()
						.expect("unreachable")
						.replace(Ok(u))
						.is_none());
					(propagation, ())
				} else {
					(Propagation::Halt, ())
				}
			}
		});
		async move {
			f.boxed().await.ok();
			Arc::try_unwrap(shelf)
				.map_err(|_| ())
				.expect("unreachable")
				.into_inner()
				.expect("unreachable")
				.expect("unreachable")
		}
	}

	fn change_blocking(&self, new_value: T) -> Result<(), T>
	where
		T: Sized + PartialEq,
	{
		self.update_blocking(|mut value| {
			if *value != new_value {
				(Propagation::Propagate, Ok(value.set(new_value)))
			} else {
				(Propagation::Halt, Err(new_value))
			}
		})
	}

	fn set_blocking(&self, new_value: T) -> ()
	where
		T: Sized,
	{
		self.update_blocking(|mut value| (Propagation::Propagate, value.set(new_value)))
	}

	fn update_blocking<U>(&self, update: impl FnOnce(Pin<&mut T>) -> (Propagation, U)) -> U
	where
		Self: Sized,
	{
		unsafe { as_inner(self) }
			.update_blocking(|value| unsafe { update(Pin::new_unchecked(value)) })
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

	use super::EnableValuePinAPI;

	pub trait Sealed {}
	impl<T> Sealed for Pin<T> where T: EnableValuePinAPI {}

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
use private::Sealed;

/// Duplicated to avoid identities.
mod private2 {
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
