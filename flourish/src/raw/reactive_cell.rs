use std::{
	borrow::Borrow,
	fmt::{self, Debug, Formatter},
	mem,
	pin::Pin,
	sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use isoprenoid::{
	raw::{Callbacks, RawSignal},
	runtime::{CallbackTableTypes, SignalRuntimeRef, Update},
};
use pin_project::pin_project;

use super::{Source, SourceCell, Subscribable};

#[pin_project]
#[repr(transparent)]
pub struct ReactiveCell<
	T: ?Sized + Send,
	H: Send + FnMut(<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
	SR: SignalRuntimeRef,
> {
	#[pin]
	signal: RawSignal<AssertSync<(Mutex<H>, RwLock<T>)>, (), SR>,
}

impl<
		T: ?Sized + Send + Debug,
		H: Send + FnMut(<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus) + Debug,
		SR: SignalRuntimeRef + Debug,
	> Debug for ReactiveCell<T, H, SR>
where
	SR::Symbol: Debug,
{
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		f.debug_struct("ReactiveCell")
			.field("signal", &&self.signal)
			.finish()
	}
}

/// TODO: Safety.
unsafe impl<
		T: ?Sized + Send,
		H: Send + FnMut(<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
		SR: SignalRuntimeRef + Sync,
	> Sync for ReactiveCell<T, H, SR>
{
}

struct AssertSync<T: ?Sized>(T);
unsafe impl<T: ?Sized> Sync for AssertSync<T> {}

impl<T: Debug + ?Sized, H: Debug> Debug for AssertSync<(Mutex<H>, RwLock<T>)> {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		let debug_tuple = &mut f.debug_tuple("AssertSync");
		{
			let maybe_guard = self.0 .1.try_read();
			debug_tuple.field(
				maybe_guard
					.as_ref()
					.map_or_else(|_| &"(locked)" as &dyn Debug, |guard| guard),
			);
		}
		{
			let maybe_guard = self.0 .0.try_lock();
			debug_tuple.field(
				maybe_guard
					.as_ref()
					.map_or_else(|_| &"(locked)" as &dyn Debug, |guard| guard),
			);
		}
		debug_tuple.finish()
	}
}

struct ReactiveCellGuard<'a, T: ?Sized>(RwLockReadGuard<'a, T>);
struct ReactiveCellGuardExclusive<'a, T: ?Sized>(RwLockWriteGuard<'a, T>);

impl<'a, T: ?Sized> Borrow<T> for ReactiveCellGuard<'a, T> {
	fn borrow(&self) -> &T {
		self.0.borrow()
	}
}

impl<'a, T: ?Sized> Borrow<T> for ReactiveCellGuardExclusive<'a, T> {
	fn borrow(&self) -> &T {
		self.0.borrow()
	}
}

impl<
		T: ?Sized + Send,
		H: Send + FnMut(<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
		SR: SignalRuntimeRef,
	> ReactiveCell<T, H, SR>
{
	pub fn new(initial_value: T, on_subscribed_status_change_fn_pin: H) -> Self
	where
		T: Sized,
		SR: Default,
	{
		Self::with_runtime(
			initial_value,
			on_subscribed_status_change_fn_pin,
			SR::default(),
		)
	}

	pub fn with_runtime(
		initial_value: T,
		on_subscribed_status_change_fn_pin: H,
		runtime: SR,
	) -> Self
	where
		T: Sized,
	{
		Self {
			signal: RawSignal::with_runtime(
				AssertSync((
					Mutex::new(on_subscribed_status_change_fn_pin),
					RwLock::new(initial_value),
				)),
				runtime,
			),
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
		ReactiveCellGuard(this.touch().read().unwrap())
	}

	pub fn read_exclusive<'a>(&'a self) -> impl 'a + Borrow<T> {
		let this = &self;
		ReactiveCellGuardExclusive(this.touch().write().unwrap())
	}

	pub fn get_mut<'a>(&'a mut self) -> &mut T {
		self.signal.eager_mut().0 .1.get_mut().unwrap()
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
				.project_or_init::<E>(|_, slot| slot.write(()))
				.0
				 .0
				 .1 as *const _)
		}
	}

	//TODO: Revisit.
	pub fn as_source_setter<'a, S>(
		self: Pin<&'a Self>,
		as_setter: impl FnOnce(Pin<&'a Self>) -> S,
	) -> (Pin<&'a impl Source<SR, Output = T>>, S)
	where
		T: Sized,
	{
		(self, as_setter(self))
	}

	//TODO: Revisit.
	pub fn as_getter_setter<'a, S, R>(
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

enum E {}
impl<
		T: ?Sized + Send,
		H: Send + FnMut(<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
		SR: SignalRuntimeRef,
	> Callbacks<AssertSync<(Mutex<H>, RwLock<T>)>, (), SR> for E
{
	const UPDATE: Option<
		fn(
			eager: Pin<&AssertSync<(Mutex<H>, RwLock<T>)>>,
			lazy: Pin<&()>,
		) -> isoprenoid::runtime::Update,
	> = None;

	const ON_SUBSCRIBED_CHANGE: Option<
		fn(
			signal: Pin<&RawSignal<AssertSync<(Mutex<H>, RwLock<T>)>, (), SR>>,
			eager: Pin<&AssertSync<(Mutex<H>, RwLock<T>)>>,
			lazy: Pin<&()>,
			subscribed: <<SR as SignalRuntimeRef>::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
		),
	> = {
		fn on_subscribed_status_change_fn_pin<
			T: ?Sized + Send,
			H: Send + FnMut( <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
			SR: SignalRuntimeRef,
		>(_: Pin<&RawSignal<AssertSync<(Mutex<H>, RwLock<T>)>, (), SR>>,eager: Pin<&AssertSync<(Mutex<H>, RwLock<T>)>>, _ :Pin<&()>, status: <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus){
			eager.0.0.lock().unwrap()(status)
		}

		Some(on_subscribed_status_change_fn_pin::<T,H,SR>)
	};
}

impl<
		T: ?Sized + Send,
		H: Send + FnMut(<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
		SR: SignalRuntimeRef,
	> Source<SR> for ReactiveCell<T, H, SR>
{
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

impl<
		T: ?Sized + Send,
		H: Send + FnMut(<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
		SR: SignalRuntimeRef,
	> Subscribable<SR> for ReactiveCell<T, H, SR>
{
	fn subscribe_inherently<'r>(self: Pin<&'r Self>) -> Option<Box<dyn 'r + Borrow<Self::Output>>> {
		//FIXME: This is inefficient.
		if self
			.project_ref()
			.signal
			.subscribe_inherently::<E>(|_, slot| slot.write(()))
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

impl<
		T: ?Sized + Send,
		H: Send + FnMut(<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
		SR: ?Sized + SignalRuntimeRef<Symbol: Sync>,
	> SourceCell<T, SR> for ReactiveCell<T, H, SR>
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
			.update(|value, _| update(&mut value.0 .1.write().unwrap()))
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
			.update_async(|value, _| update(&mut value.0 .1.write().unwrap()))
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
			.update_blocking(|value, _| update(&mut value.0 .1.write().unwrap()))
	}
}
