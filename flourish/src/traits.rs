use std::{borrow::Borrow, future::Future, ops::Deref, pin::Pin};

use isoprenoid::runtime::{Propagation, SignalsRuntimeRef};

/// **Combinators should implement this.** Interface for "raw" (stack-pinnable) signals that have an accessible value.
///
/// # Safety Notes
///
/// It's sound to transmute [`dyn Source<_>`](`Source`) between different associated `Value`s as long as that's sound and they're ABI-compatible.
///
/// Note that dropping the [`dyn Source<_>`](`Source`) dynamically **transmutes back**.
pub trait UnmanagedSignal<T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef>: Send + Sync {
	/// Records `self` as dependency without accessing the value.
	fn touch(self: Pin<&Self>);

	/// Records `self` as dependency and retrieves a copy of the value.
	///
	/// Prefer [`Source::touch`] where possible.
	fn get(self: Pin<&Self>) -> T
	where
		T: Sync + Copy,
	{
		self.get_clone()
	}

	/// Records `self` as dependency and retrieves a clone of the value.
	///
	/// Prefer [`Source::get`] where available.
	fn get_clone(self: Pin<&Self>) -> T
	where
		T: Sync + Clone;

	/// Records `self` as dependency and retrieves a copy of the value.
	///
	/// Prefer [`Source::get`] where available.
	fn get_exclusive(self: Pin<&Self>) -> T
	where
		T: Copy,
	{
		self.get_clone_exclusive()
	}

	/// Records `self` as dependency and retrieves a clone of the value.
	///
	/// Prefer [`Source::get_clone`] where available.
	fn get_clone_exclusive(self: Pin<&Self>) -> T
	where
		T: Clone;

	/// Records `self` as dependency and allows borrowing the value.
	fn read<'r>(self: Pin<&'r Self>) -> Self::Read<'r>
	where
		Self: Sized,
		T: 'r + Sync;

	type Read<'r>: 'r + Guard<T>
	where
		Self: 'r + Sized,
		T: 'r + Sync;

	/// Records `self` as dependency and allows borrowing the value.
	///
	/// Prefer [`Source::read`] where available.
	fn read_exclusive<'r>(self: Pin<&'r Self>) -> Self::ReadExclusive<'r>
	where
		Self: Sized,
		T: 'r;

	type ReadExclusive<'r>: 'r + Guard<T>
	where
		Self: 'r + Sized,
		T: 'r;

	/// The same as [`Source::read`], but object-safe.
	fn read_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r + Sync;

	/// The same as [`Source::read_exclusive`], but object-safe.
	///
	/// Prefer [`Source::read_dyn`] where available.
	fn read_exclusive_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r;

	/// Clones this [`SourcePin`]'s [`SignalsRuntimeRef`].
	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized;
}

/// **Combinators should implement this.** Allows [`SignalSR`](`crate::SignalSR`) and [`SubscriptionSR`](`crate::SubscriptionSR`) to manage subscriptions through conversions between each other.
pub trait Subscribable<T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef>:
	Send + Sync + UnmanagedSignal<T, SR>
{
	//TODO: Update docs here!

	/// Subscribes this [`Subscribable`] intrinsically.
	///
	/// If necessary, this instance is initialised first, so that callbacks are active for it.
	///
	/// # Logic
	///
	/// The implementor **must** ensure dependencies are evaluated and current iff [`Some`] is returned.
	///
	/// Iff this method is called in parallel, initialising and subscribing calls **may** differ!
	fn subscribe(self: Pin<&Self>);

	/// Unsubscribes this [`Subscribable`] intrinsically.
	///
	/// # Logic
	///
	/// Iff this isn't balanced with previous [`.subscribe()`](`Subscribable::subscribe`)
	/// calls on this instance, the runtime **should** panic and **may** exhibit
	/// unexpected behaviour (but not undefined behaviour).
	fn unsubscribe(self: Pin<&Self>);
}

/// [`Cell`](`core::cell::Cell`)-likes that announce changes to their values to a [`SignalsRuntimeRef`].
///
/// The "update" and "async" methods are non-dispatchable (meaning they can't be called on trait objects).
pub trait UnmanagedSignalCell<T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef>:
	Send + Sync + Subscribable<T, SR>
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

	/// The same as [`update`](`UnmanagedSignalCell::update`), but object-safe.
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

	/// The same as [`change_eager`](`UnmanagedSignalCell::change_eager`), but object-safe.
	fn change_eager_dyn<'f>(
		self: Pin<&Self>,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<Result<T, T>, T>>>
	where
		T: 'f + Sized + PartialEq;

	/// The same as [`replace_eager`](`UnmanagedSignalCell::replace_eager`), but object-safe.
	fn replace_eager_dyn<'f>(
		self: Pin<&Self>,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<T, T>>>
	where
		T: 'f + Sized;

	/// The same as [`update_eager`](`UnmanagedSignalCell::update_eager`), but object-safe.
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

	/// The same as [`update_blocking`](`UnmanagedSignalCell::update_blocking`), but object-safe.
	fn update_blocking_dyn(&self, update: Box<dyn '_ + FnOnce(&mut T) -> Propagation>);

	fn as_source_and_cell(
		self: Pin<&Self>,
	) -> (
		Pin<&impl UnmanagedSignal<T, SR>>,
		Pin<&impl UnmanagedSignalCell<T, SR>>,
	)
	where
		Self: Sized,
	{
		(self, self)
	}
}

//FIXME: This really should just specify `Borrow<Self::Target>`,
//       but that makes it not object-safe currently.
pub trait Guard<T: ?Sized>: Deref<Target = T> + Borrow<T> {}
