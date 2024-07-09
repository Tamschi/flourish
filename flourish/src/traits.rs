use std::{borrow::Borrow, future::Future, mem, pin::Pin};

use isoprenoid::runtime::{SignalRuntimeRef, Update};

/// **Combinators should implement this.** Interface for "raw" (stack-pinnable) signals that have an accessible value.
///
/// # Safety Notes
///
/// It's sound to transmute [`dyn Source<_>`](`Source`) between different associated `Value`s as long as that's sound and they're ABI-compatible.
///
/// Note that dropping the [`dyn Source<_>`](`Source`) dynamically **transmutes back**.
pub trait Source<SR: ?Sized + SignalRuntimeRef>: Send + Sync {
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

/// **Most application code should consume this.** Interface for movable signal handles that have an accessible value.
pub trait SourcePin<SR: ?Sized + SignalRuntimeRef>: Send + Sync {
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

/// **Combinators should implement this.** Allows [`SignalSR`](`crate::SignalSR`) and [`SubscriptionSR`](`crate::SubscriptionSR`) to manage subscriptions through conversions between each other.
pub trait Subscribable<SR: ?Sized + SignalRuntimeRef>: Send + Sync + Source<SR> {
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

/// [`Cell`](`core::cell::Cell`)-likes that announce changes to their values to a [`SignalRuntimeRef`].
///
/// The "update" and "async" methods are non-dispatchable (meaning they can't be called on trait objects).
pub trait SourceCell<T: ?Sized + Send, SR: ?Sized + SignalRuntimeRef<Symbol: Sync>>:
	Send + Sync + Subscribable<SR>
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
	fn update(self: Pin<&Self>, update: impl 'static + Send + FnOnce(&mut T) -> Update)
	where
		Self: Sized,
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
	/// This method **should not** apply its effect unless the returned [`Future`] is polled.  
	/// The returned [`Future`] **may** return [`Pending`](`core::task::Poll::Pending`) indefinitely iff polled in signal callbacks.
	///
	/// Don't `.await` the returned [`Future`] in signal callbacks!
	fn change_async(self: Pin<&Self>, new_value: T) -> impl Send + Future<Output = Result<T, T>>
	where
		Self: Sized,
		T: Sized + PartialEq,
	{
		self.update_async(|value| {
			if *value != new_value {
				(Ok(mem::replace(value, new_value)), Update::Propagate)
			} else {
				(Err(new_value), Update::Halt)
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
	fn replace_async(self: Pin<&Self>, new_value: T) -> impl Send + Future<Output = T>
	where
		Self: Sized,
		T: Sized,
	{
		self.update_async(|value| (mem::replace(value, new_value), Update::Propagate))
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
		self: Pin<&Self>,
		update: impl Send + FnOnce(&mut T) -> (U, Update),
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
	fn update_blocking<U>(&self, update: impl FnOnce(&mut T) -> (U, Update)) -> U
	where
		Self: Sized;
}
