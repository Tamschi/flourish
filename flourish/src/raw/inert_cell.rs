use std::{
	borrow::Borrow,
	fmt::{self, Debug, Formatter},
	mem,
	pin::Pin,
	sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use isoprenoid::{
	raw::{NoCallbacks, RawSignal},
	runtime::{SignalRuntimeRef, Update},
};
use pin_project::pin_project;

use super::{Source, SourceCell, Subscribable};

#[pin_project]
pub struct InertCell<T: ?Sized + Send, SR: SignalRuntimeRef> {
	#[pin]
	signal: RawSignal<AssertSync<RwLock<T>>, (), SR>,
}

impl<T: ?Sized + Send + Debug, SR: SignalRuntimeRef + Debug> Debug for InertCell<T, SR>
where
	SR::Symbol: Debug,
{
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		f.debug_struct("InertCell")
			.field("signal", &&self.signal)
			.finish()
	}
}

/// TODO: Safety.
unsafe impl<T: Send + ?Sized, SR: SignalRuntimeRef + Sync> Sync for InertCell<T, SR> {}

struct AssertSync<T: ?Sized>(T);
unsafe impl<T: ?Sized> Sync for AssertSync<T> {}

impl<T: Debug + ?Sized> Debug for AssertSync<RwLock<T>> {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		let maybe_guard = self.0.try_write();
		f.debug_tuple("AssertSync")
			.field(
				maybe_guard
					.as_ref()
					.map_or_else(|_| &"(locked)" as &dyn Debug, |guard| guard),
			)
			.finish()
	}
}

struct InertCellGuard<'a, T: ?Sized>(RwLockReadGuard<'a, T>);
struct InertCellGuardExclusive<'a, T: ?Sized>(RwLockWriteGuard<'a, T>);

impl<'a, T: ?Sized> Borrow<T> for InertCellGuard<'a, T> {
	fn borrow(&self) -> &T {
		self.0.borrow()
	}
}

impl<'a, T: ?Sized> Borrow<T> for InertCellGuardExclusive<'a, T> {
	fn borrow(&self) -> &T {
		self.0.borrow()
	}
}

impl<T: ?Sized + Send, SR: SignalRuntimeRef> InertCell<T, SR> {
	pub fn new(initial_value: T) -> Self
	where
		T: Sized,
		SR: Default,
	{
		Self::with_runtime(initial_value, SR::default())
	}

	pub fn with_runtime(initial_value: T, runtime: SR) -> Self
	where
		T: Sized,
	{
		Self {
			signal: RawSignal::with_runtime(AssertSync(RwLock::new(initial_value)), runtime),
		}
	}

	pub fn get(&self) -> T
	where
		T: Sync + Copy,
	{
		*self.read().borrow()
	}

	pub fn get_clone(&self) -> T
	where
		T: Sync + Clone,
	{
		self.read().borrow().clone()
	}

	pub fn read<'a>(&'a self) -> impl 'a + Borrow<T>
	where
		T: Sync,
	{
		let this = &self;
		InertCellGuard(this.touch().read().unwrap())
	}

	pub fn read_exclusive<'a>(&'a self) -> impl 'a + Borrow<T> {
		let this = &self;
		InertCellGuardExclusive(this.touch().write().unwrap())
	}

	pub fn get_mut<'a>(&'a mut self) -> &mut T {
		self.signal.eager_mut().0.get_mut().unwrap()
	}

	pub fn get_exclusive(&self) -> T
	where
		T: Copy,
	{
		self.get_clone_exclusive()
	}

	pub fn get_clone_exclusive(&self) -> T
	where
		T: Clone,
	{
		self.touch().write().unwrap().clone()
	}

	pub(crate) fn touch(&self) -> &RwLock<T> {
		unsafe {
			// SAFETY: Doesn't defer memory access.
			&*(&Pin::new_unchecked(&self.signal)
				.project_or_init::<NoCallbacks>(|_, slot| slot.write(()))
				.0
				 .0 as *const _)
		}
	}

	pub fn as_source_and_setter<'a, S>(
		self: Pin<&'a Self>,
		as_setter: impl FnOnce(Pin<&'a Self>) -> S,
	) -> (Pin<&'a impl Source<SR, Output = T>>, S)
	where
		T: Sized,
	{
		(self, as_setter(self))
	}

	pub fn as_getter_and_setter<'a, S, R>(
		self: Pin<&'a Self>,
		source_as_getter: impl FnOnce(Pin<&'a dyn Source<SR, Output = T>>) -> R,
		as_setter: impl FnOnce(Pin<&'a Self>) -> S,
	) -> (R, S)
	where
		T: Sized,
	{
		(source_as_getter(self), as_setter(self))
	}
}

impl<T: Send + ?Sized, SR: SignalRuntimeRef> Source<SR> for InertCell<T, SR> {
	type Output = T;

	fn touch(self: Pin<&Self>) {
		(*self).touch();
	}

	fn get(self: Pin<&Self>) -> Self::Output
	where
		Self::Output: Sync + Copy,
	{
		(*self).get()
	}

	fn get_clone(self: Pin<&Self>) -> Self::Output
	where
		Self::Output: Sync + Clone,
	{
		(*self).get_clone()
	}

	fn get_exclusive(self: Pin<&Self>) -> Self::Output
	where
		Self::Output: Copy,
	{
		(*self).get_exclusive()
	}

	fn get_clone_exclusive(self: Pin<&Self>) -> Self::Output
	where
		Self::Output: Clone,
	{
		(*self).get_clone_exclusive()
	}

	fn read<'a>(self: Pin<&'a Self>) -> Box<dyn 'a + Borrow<Self::Output>>
	where
		Self::Output: Sync,
	{
		Box::new(self.get_ref().read())
	}

	fn read_exclusive<'a>(self: Pin<&'a Self>) -> Box<dyn 'a + Borrow<Self::Output>> {
		Box::new(self.get_ref().read_exclusive())
	}

	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		self.signal.clone_runtime_ref()
	}
}

impl<T: Send + ?Sized, SR: ?Sized + SignalRuntimeRef> Subscribable<SR> for InertCell<T, SR> {
	fn subscribe_inherently<'r>(self: Pin<&'r Self>) -> Option<Box<dyn 'r + Borrow<Self::Output>>> {
		//FIXME: This is inefficient.
		if self
			.project_ref()
			.signal
			.subscribe_inherently::<NoCallbacks>(|_, slot| slot.write(()))
			.is_some()
		{
			Some(self.read_exclusive())
		} else {
			None
		}
	}

	fn unsubscribe_inherently(self: Pin<&Self>) -> bool {
		self.project_ref().signal.unsubscribe_inherently()
	}
}

impl<T: Send + ?Sized, SR: ?Sized + SignalRuntimeRef<Symbol: Sync>> SourceCell<T, SR>
	for InertCell<T, SR>
{
	fn change(self: Pin<&Self>, new_value: T)
	where
		T: 'static + Sized + PartialEq,
		SR::Symbol: Sync,
	{
		self.update(|value| {
			if *value != new_value {
				*value = new_value;
				Update::Propagate
			} else {
				Update::Halt
			}
		});
	}

	fn replace(self: Pin<&Self>, new_value: T)
	where
		T: 'static + Sized,
		SR::Symbol: Sync,
	{
		self.update(|value| {
			*value = new_value;
			Update::Propagate
		});
	}

	fn update(self: Pin<&Self>, update: impl 'static + Send + FnOnce(&mut T) -> Update) {
		self.signal
			.clone_runtime_ref()
			.run_detached(|| self.touch());
		self.project_ref()
			.signal
			.update(|value, _| update(&mut value.0.write().unwrap()))
	}

	async fn update_async<U: Send>(
		self: Pin<&Self>,
		update: impl Send + FnOnce(&mut T) -> (U, Update),
	) -> U {
		self.signal
			.clone_runtime_ref()
			.run_detached(|| self.touch());
		self.project_ref()
			.signal
			.update_async(|value, _| update(&mut value.0.write().unwrap()))
			.await
	}

	fn change_blocking(&self, new_value: T) -> Result<T, T>
	where
		T: Sized + PartialEq,
	{
		self.update_blocking(|value| {
			if *value != new_value {
				(Ok(mem::replace(value, new_value)), Update::Propagate)
			} else {
				(Err(new_value), Update::Halt)
			}
		})
	}

	fn replace_blocking(&self, new_value: T) -> T
	where
		T: Sized,
	{
		self.update_blocking(|value| (mem::replace(value, new_value), Update::Propagate))
	}

	fn update_blocking<U>(&self, update: impl FnOnce(&mut T) -> (U, Update)) -> U {
		self.signal
			.clone_runtime_ref()
			.run_detached(|| self.touch());
		self.signal
			.update_blocking(|value, _| update(&mut value.0.write().unwrap()))
	}
}
