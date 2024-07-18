use std::{borrow::Borrow, future::Future, mem, pin::Pin};

use isoprenoid::runtime::{Propagation, SignalRuntimeRef};

pub unsafe trait EnableValuePinAPI {}

mod private {
	use std::pin::Pin;

	use super::EnableValuePinAPI;

	pub trait Sealed {}
	impl<T> Sealed for Pin<T> where T: EnableValuePinAPI {}
}
use private::Sealed;

use crate::traits::Source;

/// **Combinators should implement this.** Interface for "raw" (stack-pinnable) signals that have an accessible value.
///
/// # Safety Notes
///
/// It's sound to transmute [`dyn Source<_>`](`Source`) between different associated `Value`s as long as that's sound and they're ABI-compatible.
///
/// Note that dropping the [`dyn Source<_>`](`Source`) dynamically **transmutes back**.
pub trait PinningSource<SR: ?Sized + SignalRuntimeRef>: Send + Sync + Sealed {
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
	fn read<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Borrow<Self::Output>>
	where
		Self::Output: Sync;

	/// Records `self` as dependency and allows borrowing the value.
	///
	/// Prefer a type-associated `.read()` method where available.  
	/// Otherwise, prefer a type-associated `.read_exclusive()` method where available.  
	/// Otherwise, prefer [`Source::read`] where available.
	fn read_exclusive<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Borrow<Self::Output>>;

	/// Clones this [`SourcePin`]'s [`SignalRuntimeRef`].
	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized;
}

impl<SR: ?Sized + SignalRuntimeRef, S: Source<SR> + EnableValuePinAPI> PinningSource<SR>
	for Pin<S>
{
	type Output = S::Output;

	fn touch(self: Pin<&Self>) {
		unsafe { self.map_unchecked(|r| mem::transmute::<&Pin<S>, &S>(r)) }.touch()
	}

	fn get_clone(self: Pin<&Self>) -> Self::Output
	where
		Self::Output: Sync + Clone,
	{
		unsafe { self.map_unchecked(|r| mem::transmute::<&Pin<S>, &S>(r)) }.get_clone()
	}

	fn get_clone_exclusive(self: Pin<&Self>) -> Self::Output
	where
		Self::Output: Clone,
	{
		unsafe { self.map_unchecked(|r| mem::transmute::<&Pin<S>, &S>(r)) }.get_clone_exclusive()
	}

	fn read<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Borrow<Self::Output>>
	where
		Self::Output: Sync,
	{
		todo!()
	}

	fn read_exclusive<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Borrow<Self::Output>> {
		todo!()
	}

	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		unsafe { mem::transmute::<&Pin<S>, &S>(self) }.clone_runtime_ref()
	}
}

/// **Most application code should consume this.** Interface for movable signal handles that have an accessible value.
pub trait PinningSourcePin<SR: ?Sized + SignalRuntimeRef>: Send + Sync + Sealed {
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
	fn read<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Output>>
	where
		Self::Output: 'r + Sync;

	/// Records `self` as dependency and allows borrowing the value.
	///
	/// Prefer a type-associated `.read()` method where available.  
	/// Otherwise, prefer a type-associated `.read_exclusive()` method where available.  
	/// Otherwise, prefer [`SourcePin::read`] where available.
	fn read_exclusive<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Output>>;

	/// Clones this [`SourcePin`]'s [`SignalRuntimeRef`].
	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized;
}

//TODO: Adjust this!
/// [`Cell`](`core::cell::Cell`)-likes that announce changes to their values to a [`SignalRuntimeRef`].
///
/// The "update" and "async" methods are non-dispatchable (meaning they can't be called on trait objects).
pub trait PinningSourceCell<T: ?Sized + Send, SR: ?Sized + SignalRuntimeRef<Symbol: Sync>>:
	Send + Sync + PinningSource<SR, Output = T> + Sealed
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
		SR::Symbol: Sync;

	//TODO: `_dyn` methods?

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
	/// This method **should not** apply its effect unless the returned [`Future`] is polled.  
	/// The returned [`Future`] **may** return [`Pending`](`core::task::Poll::Pending`) indefinitely iff polled in signal callbacks.
	///
	/// Don't `.await` the returned [`Future`] in signal callbacks!
	fn change_async(&self, new_value: T) -> impl Send + Future<Output = Result<T, T>>
	where
		Self: Sized,
		T: Sized + PartialEq,
	{
		self.update_async(|value| {
			if *value != new_value {
				(Ok(mem::replace(value, new_value)), Propagation::Propagate)
			} else {
				(Err(new_value), Propagation::Halt)
			}
		})
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
	/// This method **should not** apply its effect unless the returned [`Future`] is polled.  
	/// The returned [`Future`] **may** return [`Pending`](`core::task::Poll::Pending`) indefinitely iff polled in signal callbacks.
	///
	/// Don't `.await` the returned [`Future`] in signal callbacks!
	fn replace_async(&self, new_value: T) -> impl Send + Future<Output = T>
	where
		Self: Sized,
		T: Sized,
	{
		self.update_async(|value| (mem::replace(value, new_value), Propagation::Propagate))
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
	/// This method **should not** apply its effect unless the returned [`Future`] is polled.  
	/// The returned [`Future`] **may** return [`Pending`](`core::task::Poll::Pending`) indefinitely iff polled in signal callbacks.
	///
	/// Don't `.await` the returned [`Future`] in signal callbacks!
	fn update_async<U: Send>(
		&self,
		update: impl Send + FnOnce(&mut T) -> (U, Propagation),
	) -> impl Send + Future<Output = U>
	where
		Self: Sized;

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
	fn update_blocking<U>(&self, update: impl FnOnce(&mut T) -> (U, Propagation)) -> U
	where
		Self: Sized;

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

/// [`Cell`](`core::cell::Cell`)-likes that announce changes to their values to a [`SignalRuntimeRef`].
///
/// The "update" and "async" methods are non-dispatchable (meaning they can't be called on trait objects).
pub trait PinningSourceCellPin<T: ?Sized + Send, SR: ?Sized + SignalRuntimeRef<Symbol: Sync>>:
	Send + Sync + PinningSourcePin<SR, Output = T> + Sealed
{
	/// Iff `new_value` differs from the current value, overwrites it and signals dependents.
	///
	/// # Logic
	///
	/// This method **must not** block *indefinitely*.  
	/// This method **may** defer its effect.
	fn change(&self, new_value: T)
	where
		T: 'static + Sized + PartialEq;

	/// Unconditionally overwrites the current value with `new_value` and signals dependents.
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
	fn update(&self, update: impl 'static + Send + FnOnce(Pin<&mut T>) -> Propagation)
	where
		Self: Sized,
		SR::Symbol: Sync;

	//TODO: `_dyn` methods?

	/// Iff `new_value` differs from the current value, overwrites it and signals dependents.
	///
	/// # Returns
	///
	/// [`Ok`], or [`Err(new_value)`](`Err`) iff not overwritten.
	///
	/// # Panics
	///
	/// The returned [`Future`] **may** panic if polled in signal callbacks.
	///
	/// # Logic
	///
	/// This method **must not** block *indefinitely*.  
	/// This method **should not** apply its effect unless the returned [`Future`] is polled.  
	/// The returned [`Future`] **may** return [`Pending`](`core::task::Poll::Pending`) indefinitely iff polled in signal callbacks.
	///
	/// Don't `.await` the returned [`Future`] in signal callbacks!
	fn change_async(&self, new_value: T) -> impl Send + Future<Output = Result<(), T>>
	where
		Self: Sized,
		T: Sized + PartialEq,
	{
		self.update_async(|mut value| {
			if *value != new_value {
				(Ok(value.set(new_value)), Propagation::Propagate)
			} else {
				(Err(new_value), Propagation::Halt)
			}
		})
	}

	/// Unconditionally overwrites the current value with `new_value` and signals dependents.
	///
	/// # Panics
	///
	/// The returned [`Future`] **may** panic if polled in signal callbacks.
	///
	/// # Logic
	///
	/// This method **must not** block *indefinitely*.  
	/// This method **should not** apply its effect unless the returned [`Future`] is polled.  
	/// The returned [`Future`] **may** return [`Pending`](`core::task::Poll::Pending`) indefinitely iff polled in signal callbacks.
	///
	/// Don't `.await` the returned [`Future`] in signal callbacks!
	fn replace_async(&self, new_value: T) -> impl Send + Future<Output = ()>
	where
		Self: Sized,
		T: Sized,
	{
		self.update_async(|mut value| (value.set(new_value), Propagation::Propagate))
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
	/// This method **should not** apply its effect unless the returned [`Future`] is polled.  
	/// The returned [`Future`] **may** return [`Pending`](`core::task::Poll::Pending`) indefinitely iff polled in signal callbacks.
	///
	/// Don't `.await` the returned [`Future`] in signal callbacks!
	fn update_async<U: Send>(
		&self,
		update: impl Send + FnOnce(Pin<&mut T>) -> (U, Propagation),
	) -> impl Send + Future<Output = U>
	where
		Self: Sized;

	/// Iff `new_value` differs from the current value, overwrites it and signals dependents.
	///
	/// # Returns
	///
	/// [`Ok`], or [`Err(new_value)`](`Err`) iff not overwritten.
	///
	/// # Panics
	///
	/// This method **may** panic if called in signal callbacks.
	///
	/// # Logic
	///
	/// This method **may** block *indefinitely* iff called in signal callbacks.
	fn change_blocking(&self, new_value: T) -> Result<(), T>
	where
		T: Sized + PartialEq;

	/// Unconditionally overwrites the current value with `new_value` and signals dependents.
	///
	/// # Panics
	///
	/// This method **may** panic if called in signal callbacks.
	///
	/// # Logic
	///
	/// This method **may** block *indefinitely* iff called in signal callbacks.
	fn replace_blocking(&self, new_value: T) -> ()
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
	fn update_blocking<U>(&self, update: impl FnOnce(Pin<&mut T>) -> (U, Propagation)) -> U
	where
		Self: Sized;
}
