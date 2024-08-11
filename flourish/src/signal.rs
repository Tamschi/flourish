use std::{
	borrow::Borrow,
	cell::UnsafeCell,
	fmt::{self, Debug, Formatter},
	future::Future,
	marker::PhantomData,
	mem::ManuallyDrop,
	ops::Deref,
	pin::Pin,
	process::abort,
	sync::atomic::{AtomicUsize, Ordering},
};

use isoprenoid::runtime::{Propagation, SignalsRuntimeRef};

use crate::{
	traits::{Subscribable, UnmanagedSignalCell},
	Guard,
};

pub struct Signal<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
{
	inner: UnsafeCell<Signal_<T, S, SR>>,
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Signal<T, S, SR>
{
	fn inner(&self) -> &Signal_<T, S, SR> {
		unsafe { &*self.inner.get().cast_const() }
	}

	unsafe fn inner_mut(&mut self) -> &mut Signal_<T, S, SR> {
		self.inner.get_mut()
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Debug
	for Signal<T, S, SR>
where
	S: Debug,
{
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		f.debug_tuple("Signal")
			.field(&&*self.inner().managed)
			.finish()
	}
}

pub(crate) struct Signal_<
	T: ?Sized + Send,
	S: ?Sized + Subscribable<T, SR>,
	SR: ?Sized + SignalsRuntimeRef,
> {
	_phantom: PhantomData<(PhantomData<T>, SR)>,
	strong: AtomicUsize,
	weak: AtomicUsize,
	managed: ManuallyDrop<S>,
}

pub(crate) struct Strong<
	T: ?Sized + Send,
	S: ?Sized + Subscribable<T, SR>,
	SR: ?Sized + SignalsRuntimeRef,
> {
	strong: *const Signal<T, S, SR>,
}

pub(crate) struct Weak<
	T: ?Sized + Send,
	S: ?Sized + Subscribable<T, SR>,
	SR: ?Sized + SignalsRuntimeRef,
> {
	weak: *const Signal<T, S, SR>,
}

/// # Safety
///
/// [`Send`] and [`Sync`] bound on `S` are implied by [`Subscribable`].
unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Send
	for Signal<T, S, SR>
{
}

/// # Safety
///
/// [`Send`] and [`Sync`] bound on `S` are implied by [`Subscribable`].
unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Send
	for Signal_<T, S, SR>
{
}

/// # Safety
///
/// [`Send`] and [`Sync`] bound on `S` are implied by [`Subscribable`].
unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Send
	for Strong<T, S, SR>
{
}

/// # Safety
///
/// [`Send`] and [`Sync`] bound on `S` are implied by [`Subscribable`].
unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Send
	for Weak<T, S, SR>
{
}

/// # Safety
///
/// [`Send`] and [`Sync`] bound on `S` are implied by [`Subscribable`].
unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Sync
	for Signal<T, S, SR>
{
}

/// # Safety
///
/// [`Send`] and [`Sync`] bound on `S` are implied by [`Subscribable`].
unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Sync
	for Signal_<T, S, SR>
{
}

/// # Safety
///
/// [`Send`] and [`Sync`] bound on `S` are implied by [`Subscribable`].
unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Sync
	for Strong<T, S, SR>
{
}

/// # Safety
///
/// [`Send`] and [`Sync`] bound on `S` are implied by [`Subscribable`].
unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Sync
	for Weak<T, S, SR>
{
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Strong<T, S, SR>
{
	pub(crate) fn pin(managed: S) -> Self
	where
		S: Sized,
	{
		Self {
			strong: Box::into_raw(Box::new(Signal {
				inner: Signal_ {
					_phantom: PhantomData,
					strong: 1.into(),
					weak: 1.into(),
					managed: ManuallyDrop::new(managed),
				}
				.into(),
			})),
		}
	}

	fn get(&self) -> &Signal<T, S, SR> {
		unsafe { &*self.strong }
	}

	unsafe fn get_mut(&mut self) -> &mut Signal<T, S, SR> {
		&mut *self.strong.cast_mut()
	}

	pub(crate) fn downgrade(&self) -> Weak<T, S, SR> {
		(*ManuallyDrop::new(Weak { weak: self.strong })).clone()
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Deref
	for Strong<T, S, SR>
{
	type Target = Signal<T, S, SR>;

	fn deref(&self) -> &Self::Target {
		self.get()
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Borrow<Signal<T, S, SR>> for Strong<T, S, SR>
{
	fn borrow(&self) -> &Signal<T, S, SR> {
		self.get()
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Weak<T, S, SR>
{
	fn inner(&self) -> &Signal_<T, S, SR> {
		unsafe { &*(*self.weak).inner.get().cast_const() }
	}

	pub(crate) fn upgrade(&self) -> Option<Strong<T, S, SR>> {
		let mut strong = self.inner().strong.load(Ordering::Relaxed);
		while strong > 0 {
			match self.inner().strong.compare_exchange(
				strong,
				strong + 1,
				Ordering::Acquire,
				Ordering::Relaxed,
			) {
				Ok(_) => return Some(Strong { strong: self.weak }),
				Err(actual) => strong = actual,
			}
		}
		None
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Drop
	for Strong<T, S, SR>
{
	fn drop(&mut self) {
		if self.get().inner().strong.fetch_sub(1, Ordering::Release) == 1 {
			unsafe { ManuallyDrop::drop(&mut self.get_mut().inner_mut().managed) }
			drop(Weak { weak: self.strong })
		}
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Drop
	for Weak<T, S, SR>
{
	fn drop(&mut self) {
		if self.inner().weak.fetch_sub(1, Ordering::Release) == 1 {
			unsafe {
				drop(Box::from_raw(self.weak.cast_mut()));
			}
		}
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Clone
	for Strong<T, S, SR>
{
	fn clone(&self) -> Self {
		if self.get().inner().strong.fetch_add(1, Ordering::Relaxed) > usize::MAX / 2 {
			eprintln!("SignalArc overflow.");
			abort()
		}
		Self {
			strong: self.strong,
		}
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Clone
	for Weak<T, S, SR>
{
	fn clone(&self) -> Self {
		if self.inner().weak.fetch_add(1, Ordering::Relaxed) > usize::MAX / 2 {
			eprintln!("SignalWeak overflow.");
			abort()
		}
		Self { weak: self.weak }
	}
}

/// **Most application code should consume this.** Interface for movable signal handles that have an accessible value.
impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Signal<T, S, SR>
{
	pub(crate) fn _managed(&self) -> Pin<&S> {
		unsafe { Pin::new_unchecked(&self.inner().managed) }
	}

	/// Records `self` as dependency without accessing the value.
	fn touch(&self) {
		self._managed().touch()
	}

	/// Records `self` as dependency and retrieves a copy of the value.
	///
	/// Prefer [`SourcePin::touch`] where possible.
	fn get(&self) -> T
	where
		T: Sync + Copy,
	{
		self._managed().get()
	}

	/// Records `self` as dependency and retrieves a clone of the value.
	///
	/// Prefer [`SourcePin::get`] where available.
	fn get_clone(&self) -> T
	where
		T: Sync + Clone,
	{
		self._managed().get_clone()
	}

	/// Records `self` as dependency and retrieves a copy of the value.
	///
	/// Prefer [`SourcePin::get`] where available.
	fn get_exclusive(&self) -> T
	where
		T: Copy,
	{
		self._managed().get_clone()
	}

	/// Records `self` as dependency and retrieves a clone of the value.
	///
	/// Prefer [`SourcePin::get_clone`] where available.
	fn get_clone_exclusive(&self) -> T
	where
		T: Clone,
	{
		self._managed().get_clone_exclusive()
	}

	/// Records `self` as dependency and allows borrowing the value.
	fn read<'r>(&'r self) -> S::Read<'r>
	where
		S: Sized,
		T: 'r + Sync,
	{
		self._managed().read()
	}

	/// Records `self` as dependency and allows borrowing the value.
	///
	/// Prefer [`SourcePin::read`] where available.
	fn read_exclusive<'r>(&'r self) -> S::ReadExclusive<'r>
	where
		S: Sized,
		T: 'r,
	{
		self._managed().read_exclusive()
	}

	/// The same as [`SourcePin::read`], but dyn-compatible.
	fn read_dyn<'r>(&'r self) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r + Sync,
	{
		self._managed().read_dyn()
	}

	/// The same as [`SourcePin::read_exclusive`], but dyn-compatible.
	///
	/// Prefer [`SourcePin::read_dyn`] where available.
	fn read_exclusive_dyn<'r>(&'r self) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r,
	{
		self._managed().read_exclusive_dyn()
	}

	/// Clones this [`SourcePin`]'s [`SignalsRuntimeRef`].
	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		self._managed().clone_runtime_ref()
	}
}

/// [`Cell`](`core::cell::Cell`)-likes that announce changes to their values to a [`SignalsRuntimeRef`].
///
/// The "update" and "async" methods are non-dispatchable (meaning they can't be called on trait objects).
impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignalCell<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Signal<T, S, SR>
{
	/// Iff `new_value` differs from the current value, replaces it and signals dependents.
	///
	/// # Logic
	///
	/// This method **must not** block *indefinitely*.  
	/// This method **may** defer its effect.
	fn change(&self, new_value: T)
	where
		T: 'static + Sized + PartialEq,
	{
		self._managed().change(new_value)
	}

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
		T: 'static + Sized,
	{
		self._managed().replace(new_value)
	}

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
	{
		self._managed().update(update)
	}

	/// The same as [`update`](`UnmanagedSignalCellPin::update`), but dyn-compatible.
	fn update_dyn(&self, update: Box<dyn 'static + Send + FnOnce(&mut T) -> Propagation>)
	where
		T: 'static,
	{
		self._managed().update_dyn(update)
	}

	/// Cheaply creates a [`Future`] that has the effect of [`change_eager`](`UnmanagedSignalCellPin::change_eager`) when polled.
	///
	/// # Logic
	///
	/// The [`Future`] **should not** hold a strong reference to `self`.
	fn change_async<'f>(&self, new_value: T) -> S::ChangeAsync<'f>
	where
		S: 'f + Sized,
		T: 'f + Sized + PartialEq,
	{
		self._managed().change_async(new_value)
	}

	/// Cheaply creates a [`Future`] that has the effect of [`replace_eager`](`UnmanagedSignalCellPin::replace_eager`) when polled.
	///
	/// # Logic
	///
	/// The [`Future`] **should not** hold a strong reference to `self`.
	fn replace_async<'f>(&self, new_value: T) -> S::ReplaceAsync<'f>
	where
		S: 'f + Sized,
		T: 'f + Sized,
	{
		self._managed().replace_async(new_value)
	}

	/// Cheaply creates a [`Future`] that has the effect of [`update_eager`](`UnmanagedSignalCellPin::update_eager`) when polled.
	///
	/// # Logic
	///
	/// The [`Future`] **should not** hold a strong reference to `self`.
	fn update_async<'f, U: 'f + Send, F: 'f + Send + FnOnce(&mut T) -> (Propagation, U)>(
		&self,
		update: F,
	) -> S::UpdateAsync<'f, U, F>
	where
		S: 'f + Sized,
	{
		self._managed().update_async(update)
	}

	/// The same as [`change_async`](`UnmanagedSignalCellPin::change_async`), but dyn-compatible.
	fn change_async_dyn<'f>(
		&self,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<Result<T, T>, T>>>
	where
		T: 'f + Sized + PartialEq,
	{
		self._managed().change_async_dyn(new_value)
	}

	/// The same as [`replace_async`](`UnmanagedSignalCellPin::replace_async`), but dyn-compatible.
	fn replace_async_dyn<'f>(
		&self,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<T, T>>>
	where
		T: 'f + Sized,
	{
		self._managed().replace_async_dyn(new_value)
	}

	/// The same as [`update_async`](`UnmanagedSignalCellPin::update_async`), but dyn-compatible.
	fn update_async_dyn<'f>(
		&self,
		update: Box<dyn 'f + Send + FnOnce(&mut T) -> Propagation>,
	) -> Box<
		dyn 'f
			+ Send
			+ Future<Output = Result<(), Box<dyn 'f + Send + FnOnce(&mut T) -> Propagation>>>,
	>
	where
		T: 'f,
	{
		self._managed().update_async_dyn(update)
	}

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
	fn change_eager<'f>(&self, new_value: T) -> S::ChangeEager<'f>
	where
		S: 'f + Sized,
		T: 'f + Sized + PartialEq,
	{
		self._managed().change_eager(new_value)
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
	/// This method **should** schedule its effect even if the returned [`Future`] is not polled.  
	/// This method **should** cancel its effect when the returned [`Future`] is dropped.  
	/// The returned [`Future`] **may** return [`Pending`](`core::task::Poll::Pending`) indefinitely iff polled in signal callbacks.
	///
	/// Don't `.await` the returned [`Future`] in signal callbacks!
	fn replace_eager<'f>(&self, new_value: T) -> S::ReplaceEager<'f>
	where
		S: 'f + Sized,
		T: 'f + Sized,
	{
		self._managed().replace_eager(new_value)
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
	/// This method **should** schedule its effect even if the returned [`Future`] is not polled.  
	/// This method **should** cancel its effect when the returned [`Future`] is dropped.  
	/// The returned [`Future`] **may** return [`Pending`](`core::task::Poll::Pending`) indefinitely iff polled in signal callbacks.
	///
	/// Don't `.await` the returned [`Future`] in signal callbacks!
	fn update_eager<'f, U: Send, F: 'f + Send + FnOnce(&mut T) -> (Propagation, U)>(
		&self,
		update: F,
	) -> S::UpdateEager<'f, U, F>
	where
		S: 'f + Sized,
	{
		self._managed().update_eager(update)
	}

	/// The same as [`change_eager`](`UnmanagedSignalCellPin::change_eager`), but dyn-compatible.
	fn change_eager_dyn<'f>(
		&self,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<Result<T, T>, T>>>
	where
		T: 'f + Sized + PartialEq,
	{
		self._managed().change_eager_dyn(new_value)
	}

	/// The same as [`replace_eager`](`UnmanagedSignalCellPin::replace_eager`), but dyn-compatible.
	fn replace_eager_dyn<'f>(
		&self,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<T, T>>>
	where
		T: 'f + Sized,
	{
		self._managed().replace_eager_dyn(new_value)
	}

	/// The same as [`update_eager`](`UnmanagedSignalCellPin::update_eager`), but dyn-compatible.
	fn update_eager_dyn<'f>(
		&self,
		update: Box<dyn 'f + Send + FnOnce(&mut T) -> Propagation>,
	) -> Box<
		dyn 'f
			+ Send
			+ Future<Output = Result<(), Box<dyn 'f + Send + FnOnce(&mut T) -> Propagation>>>,
	>
	where
		T: 'f,
	{
		self._managed().update_eager_dyn(update)
	}

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
		T: Sized + PartialEq,
	{
		self._managed().change_blocking(new_value)
	}

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
		T: Sized,
	{
		self._managed().replace_blocking(new_value)
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
	/// This method **may** panic if called in signal callbacks.
	///
	/// # Logic
	///
	/// This method **may** block *indefinitely* iff called in signal callbacks.
	fn update_blocking<U>(&self, update: impl FnOnce(&mut T) -> (Propagation, U)) -> U
	where
		Self: Sized,
	{
		self._managed().update_blocking(update)
	}

	/// The same as [`update_blocking`](`UnmanagedSignalCellPin::update_blocking`), but dyn-compatible.
	fn update_blocking_dyn(&self, update: Box<dyn '_ + FnOnce(&mut T) -> Propagation>) {
		self._managed().update_blocking_dyn(update)
	}
}
