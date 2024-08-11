use std::{
	borrow::Borrow,
	cell::UnsafeCell,
	fmt::{self, Debug, Formatter},
	marker::PhantomData,
	mem::ManuallyDrop,
	ops::Deref,
	pin::Pin,
	sync::atomic::{AtomicUsize, Ordering},
};

use isoprenoid::runtime::{GlobalSignalsRuntime, SignalsRuntimeRef};

use crate::traits::Subscribable;

pub type Signal<T: ?Sized + Send, S: ?Sized + Subscribable<T, GlobalSignalsRuntime>> =
	SignalSR<T, S, GlobalSignalsRuntime>;

pub struct SignalSR<
	T: ?Sized + Send,
	S: ?Sized + Subscribable<T, SR>,
	SR: ?Sized + SignalsRuntimeRef,
> {
	inner: UnsafeCell<Signal_<T, S, SR>>,
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	SignalSR<T, S, SR>
{
	fn inner(&self) -> &Signal_<T, S, SR> {
		unsafe { &*self.inner.get().cast_const() }
	}

	unsafe fn inner_mut(&mut self) -> &mut Signal_<T, S, SR> {
		self.inner.get_mut()
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Debug
	for SignalSR<T, S, SR>
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
	strong: *const SignalSR<T, S, SR>,
}

pub(crate) struct Weak<
	T: ?Sized + Send,
	S: ?Sized + Subscribable<T, SR>,
	SR: ?Sized + SignalsRuntimeRef,
> {
	weak: *const SignalSR<T, S, SR>,
}

/// # Safety
///
/// [`Send`] and [`Sync`] bound on `S` are implied by [`Subscribable`].
unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Send
	for SignalSR<T, S, SR>
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
	for SignalSR<T, S, SR>
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
	pub(crate) fn pin(managed: S) -> Pin<Self>
	where
		S: Sized,
	{
		let new = Self {
			strong: Box::into_raw(Box::new(SignalSR {
				inner: Signal_ {
					_phantom: PhantomData,
					strong: 1.into(),
					weak: 1.into(),
					managed: ManuallyDrop::new(managed),
				}
				.into(),
			})),
		};
		unsafe { Pin::new_unchecked(new) }
	}

	fn get(&self) -> &SignalSR<T, S, SR> {
		unsafe { &*self.strong }
	}

	unsafe fn get_mut(&mut self) -> &mut SignalSR<T, S, SR> {
		&mut *self.strong.cast_mut()
	}

	pub(crate) fn downgrade(&self) -> Weak<T, S, SR> {
		(*ManuallyDrop::new(Weak { weak: self.strong })).clone()
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Deref
	for Strong<T, S, SR>
{
	type Target = SignalSR<T, S, SR>;

	fn deref(&self) -> &Self::Target {
		self.get()
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Borrow<SignalSR<T, S, SR>> for Strong<T, S, SR>
{
	fn borrow(&self) -> &SignalSR<T, S, SR> {
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
		self.get().inner().strong.fetch_add(1, Ordering::Relaxed);
		Self {
			strong: self.strong,
		}
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Clone
	for Weak<T, S, SR>
{
	fn clone(&self) -> Self {
		self.inner().weak.fetch_add(1, Ordering::Relaxed);
		Self { weak: self.weak }
	}
}
