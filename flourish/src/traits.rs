use std::{borrow::Borrow, future::Future, ops::Deref, pin::Pin};

use isoprenoid::runtime::{Propagation, SignalsRuntimeRef};

//TODO: Revise "# Returns" documentation! Some is mismatched.

/// "Unmanaged" (stack-pinnable) signals that have an accessible value.
///
/// **Combinators should implement this.**
///
/// # Safety Notes
///
/// It's sound to transmute [`dyn UnmanagedSignal<T, SR>`](`UnmanagedSignal`) between different `T`s as long as that's sound and they're ABI-compatible.
///
/// Note that dropping the [`dyn UnmanagedSignal<T, SR>`](`UnmanagedSignal`) dynamically **transmutes back** since it drops the value as the original type.
pub trait UnmanagedSignal<T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef>: Send + Sync {
	/// Records `self` as dependency without accessing the value.
	fn touch(self: Pin<&Self>);

	/// Records `self` as dependency and retrieves a copy of the value.
	///
	/// Prefer [`touch`](`UnmanagedSignal::touch`) where possible.
	fn get(self: Pin<&Self>) -> T
	where
		T: Sync + Copy,
	{
		self.get_clone()
	}

	/// Records `self` as dependency and retrieves a clone of the value.
	///
	/// Prefer [`get`](`UnmanagedSignal::get`) where available.
	fn get_clone(self: Pin<&Self>) -> T
	where
		T: Sync + Clone;

	/// Records `self` as dependency and retrieves a copy of the value.
	///
	/// Prefer [`get`](`UnmanagedSignal::get`) where available.
	fn get_exclusive(self: Pin<&Self>) -> T
	where
		T: Copy,
	{
		self.get_clone_exclusive()
	}

	/// Records `self` as dependency and retrieves a clone of the value.
	///
	/// Prefer [`get_clone`](`UnmanagedSignal::get_clone`) where available.
	fn get_clone_exclusive(self: Pin<&Self>) -> T
	where
		T: Clone;

	/// Records `self` as dependency and allows borrowing the value.
	fn read<'r>(self: Pin<&'r Self>) -> Self::Read<'r>
	where
		Self: Sized,
		T: 'r + Sync;

	/// Return type of [`read`](`UnmanagedSignal::read`).
	type Read<'r>: 'r + Guard<T>
	where
		Self: 'r + Sized,
		T: 'r + Sync;

	/// Records `self` as dependency and allows borrowing the value.
	///
	/// Prefer [`read`](`UnmanagedSignal::read`) where available.
	fn read_exclusive<'r>(self: Pin<&'r Self>) -> Self::ReadExclusive<'r>
	where
		Self: Sized,
		T: 'r;

	/// Return type of [`read_exclusive`](`UnmanagedSignal::read_exclusive`).
	type ReadExclusive<'r>: 'r + Guard<T>
	where
		Self: 'r + Sized,
		T: 'r;

	/// The same as [`read`](`UnmanagedSignal::read`), but `dyn`-compatible.
	fn read_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r + Sync;

	/// The same as [`read_exclusive`](`UnmanagedSignal::read_exclusive`), but `dyn`-compatible.
	///
	/// Prefer [`read_dyn`](`UnmanagedSignal::read_dyn`) where available.
	fn read_exclusive_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r;

	/// Subscribes this [`UnmanagedSignal`] intrinsically.
	///
	/// If necessary, this instance is initialised first, so that callbacks are active for it.
	///
	/// # Logic
	///
	/// Iff this method is called concurrently, initialising and subscribing calls **may** differ!
	fn subscribe(self: Pin<&Self>);

	/// Unsubscribes this [`UnmanagedSignal`] intrinsically.
	///
	/// # Logic
	///
	/// Iff this isn't balanced with previous [`.subscribe()`](`UnmanagedSignal::subscribe`)
	/// calls on this instance, the runtime **should** panic and **may** exhibit
	/// unexpected behaviour (but not undefined behaviour).
	fn unsubscribe(self: Pin<&Self>);

	/// Clones this [`UnmanagedSignal`]'s [`SignalsRuntimeRef`].
	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized;
}

/// [`Cell`](`core::cell::Cell`)-likes that announce changes to their values to a [`SignalsRuntimeRef`].
///
/// The "update" and "async" methods are non-dispatchable (meaning they can't be called on trait objects).
pub trait UnmanagedSignalCell<T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef>:
	Send + Sync + UnmanagedSignal<T, SR>
{
	/// Iff `new_value` differs from the current value, overwrites it and signals dependents.
	///
	/// # Logic
	///
	/// This method **must not** block *indefinitely*.  
	/// This method **may** defer its effect.
	fn set_if_distinct(self: Pin<&Self>, new_value: T)
	where
		T: 'static + Sized + PartialEq;

	/// Unconditionally overwrites the current value with `new_value` and signals dependents.
	///
	/// Prefer [`.set_if_distinct(new_value)`](`UnmanagedSignalCell::set_if_distinct`) if halting propagation is acceptable.
	///
	/// # Logic
	///
	/// This method **must not** block *indefinitely*.  
	/// This method **may** defer its effect.
	fn set(self: Pin<&Self>, new_value: T)
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

	/// The same as [`update`](`UnmanagedSignalCell::update`), but `dyn`-compatible.
	fn update_dyn(
		self: Pin<&Self>,
		update: Box<dyn 'static + Send + FnOnce(&mut T) -> Propagation>,
	) where
		T: 'static;

	/// Iff `new_value` differs from the current value, overwrites it and signals dependents.
	///
	/// # Returns
	///
	/// [`Ok`], or [`Err(new_value)`](`Err`) iff not replaced.
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
	fn set_if_distinct_eager<'f>(self: Pin<&Self>, new_value: T) -> Self::SetIfDistinctEager<'f>
	where
		Self: 'f + Sized,
		T: 'f + Sized + PartialEq;

	/// Return type of [`set_if_distinct_eager`](`UnmanagedSignalCell::set_if_distinct_eager`).
	type SetIfDistinctEager<'f>: 'f + Send + Future<Output = Result<Result<(), T>, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

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
	fn replace_if_distinct_eager<'f>(
		self: Pin<&Self>,
		new_value: T,
	) -> Self::ReplaceIfDistinctEager<'f>
	where
		Self: 'f + Sized,
		T: 'f + Sized + PartialEq;

	/// Return type of [`replace_if_distinct_eager`](`UnmanagedSignalCell::replace_if_distinct_eager`).
	type ReplaceIfDistinctEager<'f>: 'f + Send + Future<Output = Result<Result<T, T>, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	/// Unconditionally overwrites the current value with `new_value` and signals dependents.
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
	fn set_eager<'f>(self: Pin<&Self>, new_value: T) -> Self::SetEager<'f>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	/// Return type of [`replace_eager`](`UnmanagedSignalCell::replace_eager`).
	type SetEager<'f>: 'f + Send + Future<Output = Result<(), T>>
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

	/// Return type of [`replace_eager`](`UnmanagedSignalCell::replace_eager`).
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

	/// Return type of [`update_eager`](`UnmanagedSignalCell::update_eager`).
	type UpdateEager<'f, U: 'f, F: 'f>: 'f + Send + Future<Output = Result<U, F>>
	where
		Self: 'f + Sized;

	/// The same as [`set_if_distinct_eager`](`UnmanagedSignalCell::set_if_distinct_eager`), but `dyn`-compatible.
	fn set_if_distinct_eager_dyn<'f>(
		self: Pin<&Self>,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<Result<(), T>, T>>>
	where
		T: 'f + Sized + PartialEq;

	/// The same as [`replace_if_distinct_eager`](`UnmanagedSignalCell::replace_if_distinct_eager`), but `dyn`-compatible.
	fn replace_if_distinct_eager_dyn<'f>(
		self: Pin<&Self>,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<Result<T, T>, T>>>
	where
		T: 'f + Sized + PartialEq;

	/// The same as [`set_eager`](`UnmanagedSignalCell::set_eager`), but `dyn`-compatible.
	fn set_eager_dyn<'f>(
		self: Pin<&Self>,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<(), T>>>
	where
		T: 'f + Sized;

	/// The same as [`replace_eager`](`UnmanagedSignalCell::replace_eager`), but `dyn`-compatible.
	fn replace_eager_dyn<'f>(
		self: Pin<&Self>,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<T, T>>>
	where
		T: 'f + Sized;

	/// The same as [`update_eager`](`UnmanagedSignalCell::update_eager`), but `dyn`-compatible.
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

	/// Iff `new_value` differs from the current value, overwrites it and signals dependents.
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
	fn set_if_distinct_blocking(&self, new_value: T) -> Result<(), T>
	where
		T: Sized + PartialEq;

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
	fn replace_if_distinct_blocking(&self, new_value: T) -> Result<T, T>
	where
		T: Sized + PartialEq;

	/// Unconditionally overwrites the current value with `new_value` and signals dependents.
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
	fn set_blocking(&self, new_value: T)
	where
		T: Sized;

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

	/// The same as [`update_blocking`](`UnmanagedSignalCell::update_blocking`), but `dyn`-compatible.
	fn update_blocking_dyn(&self, update: Box<dyn '_ + FnOnce(&mut T) -> Propagation>);

	/// Convenience method to split a pinning reference to this [`UnmanagedSignalCell`]
	/// into a read-only/writable pair.
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

/// Read-guards returned by `read…` methods.
///
/// > **FIXME**
/// >
/// > Ideally, this trait would be:
/// >
/// > ```
/// > # use std::{borrow::Borrow, ops::Deref};
/// > // Not dyn-compatible as of Rust 1.82 ☹️
/// > pub trait Guard: Deref + Borrow<Self::Target> {}
/// > ```
/// >
/// > See: <https://github.com/rust-lang/rust/issues/65078>
pub trait Guard<T: ?Sized>: Deref<Target = T> + Borrow<T> {}
