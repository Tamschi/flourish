use std::{borrow::Borrow, future::Future, pin::Pin};

use isoprenoid::runtime::{Propagation, SignalsRuntimeRef};

/// **Combinators should implement this.** Interface for "raw" (stack-pinnable) signals that have an accessible value.
///
/// # Safety Notes
///
/// It's sound to transmute [`dyn Source<_>`](`Source`) between different associated `Value`s as long as that's sound and they're ABI-compatible.
///
/// Note that dropping the [`dyn Source<_>`](`Source`) dynamically **transmutes back**.
pub trait Source<SR: ?Sized + SignalsRuntimeRef>: Send + Sync {
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

	/// Clones this [`SourcePin`]'s [`SignalsRuntimeRef`].
	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized;
}

/// **Most application code should consume this.** Interface for movable signal handles that have an accessible value.
pub trait SourcePin<SR: ?Sized + SignalsRuntimeRef>: Send + Sync {
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

	/// Clones this [`SourcePin`]'s [`SignalsRuntimeRef`].
	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized;
}

/// **Combinators should implement this.** Allows [`SignalSR`](`crate::SignalSR`) and [`SubscriptionSR`](`crate::SubscriptionSR`) to manage subscriptions through conversions between each other.
pub trait Subscribable<SR: ?Sized + SignalsRuntimeRef>: Send + Sync + Source<SR> {
	/// Subscribes this [`Subscribable`] (only regarding innate subscription)!
	///
	/// If necessary, this instance is initialised first, so that callbacks are active for it.
	///
	/// # Logic
	///
	/// The implementor **must** ensure dependencies are evaluated and current iff [`Some`] is returned.
	///
	/// Iff this method is called in parallel, initialising and subscribing calls **may** differ!
	///
	/// # Returns
	///
	/// [`Some`] iff the inherent subscription is new, otherwise [`None`].
	fn subscribe_inherently<'r>(self: Pin<&'r Self>) -> Option<Box<dyn 'r + Borrow<Self::Output>>>;

	/// Unsubscribes this [`Subscribable`] (only regarding innate subscription!).
	///
	/// # Returns
	///
	/// Whether this instance was previously innately subscribed.
	///
	/// An innate subscription is a subscription not caused by a dependent subscriber.
	fn unsubscribe_inherently(self: Pin<&Self>) -> bool;
}

/// [`Cell`](`core::cell::Cell`)-likes that announce changes to their values to a [`SignalsRuntimeRef`].
///
/// The "update" and "async" methods are non-dispatchable (meaning they can't be called on trait objects).
pub trait SourceCell<T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef<Symbol: Sync>>:
	Send + Sync + Subscribable<SR, Output = T>
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
		T: 'static,
		SR::Symbol: Sync;

	/// The same as [`update`](`SourceCell::update`), but object-safe.
	fn update_dyn(
		self: Pin<&Self>,
		update: Box<dyn 'static + Send + FnOnce(&mut T) -> Propagation>,
	) where
		T: 'static,
		SR::Symbol: Sync;

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
	fn update_blocking_dyn(&self, update: Box<dyn '_ + FnOnce(&mut T) -> Propagation>)
	where
		SR::Symbol: Sync;

	fn as_source_and_cell(
		self: Pin<&Self>,
	) -> (
		Pin<&impl Source<SR, Output = T>>,
		Pin<&impl SourceCell<T, SR>>,
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
pub trait SourceCellPin<T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef<Symbol: Sync>>:
	Send + Sync + SourcePin<SR, Output = T>
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
		T: 'static,
		SR::Symbol: Sync;

	/// The same as [`update`](`SourceCellPin::update`), but object-safe.
	fn update_dyn(&self, update: Box<dyn 'static + Send + FnOnce(&mut T) -> Propagation>)
	where
		T: 'static,
		SR::Symbol: Sync;

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
	fn update_blocking_dyn(&self, update: Box<dyn '_ + FnOnce(&mut T) -> Propagation>)
	where
		SR::Symbol: Sync;
}
