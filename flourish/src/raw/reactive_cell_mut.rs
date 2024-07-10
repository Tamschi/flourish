use std::{
	borrow::{Borrow, BorrowMut},
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
pub struct ReactiveCellMut<
	T: ?Sized + Send,
	HandlerFnPin: Send + FnMut(&mut T, <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
	SR: SignalRuntimeRef,
> {
	#[pin]
	signal: RawSignal<AssertSync<(Mutex<HandlerFnPin>, RwLock<T>)>, (), SR>,
}

impl<
		T: ?Sized + Send + Debug,
		HandlerFnPin: Send
			+ FnMut(&mut T, <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus)
			+ Debug,
		SR: SignalRuntimeRef + Debug,
	> Debug for ReactiveCellMut<T, HandlerFnPin, SR>
where
	SR::Symbol: Debug,
{
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		f.debug_struct("ReactiveCellMut")
			.field("signal", &&self.signal)
			.finish()
	}
}

/// TODO: Safety.
unsafe impl<
		T: ?Sized + Send,
		HandlerFnPin: Send + FnMut(&mut T, <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
		SR: SignalRuntimeRef + Sync,
	> Sync for ReactiveCellMut<T, HandlerFnPin, SR>
{
}

struct AssertSync<T: ?Sized>(T);
unsafe impl<T: ?Sized> Sync for AssertSync<T> {}

impl<T: Debug + ?Sized, HandlerFnPin: Debug> Debug
	for AssertSync<(Mutex<HandlerFnPin>, RwLock<T>)>
{
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

struct ReactiveCellMutGuard<'a, T: ?Sized>(RwLockReadGuard<'a, T>);
struct ReactiveCellMutGuardExclusive<'a, T: ?Sized>(RwLockWriteGuard<'a, T>);

impl<'a, T: ?Sized> Borrow<T> for ReactiveCellMutGuard<'a, T> {
	fn borrow(&self) -> &T {
		self.0.borrow()
	}
}

impl<'a, T: ?Sized> Borrow<T> for ReactiveCellMutGuardExclusive<'a, T> {
	fn borrow(&self) -> &T {
		self.0.borrow()
	}
}

impl<
		T: ?Sized + Send,
		HandlerFnPin: Send + FnMut(&mut T, <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
		SR: SignalRuntimeRef,
	> ReactiveCellMut<T, HandlerFnPin, SR>
{
	pub(crate) fn new(initial_value: T, on_subscribed_change_fn_pin: HandlerFnPin) -> Self
	where
		T: Sized,
		SR: Default,
	{
		Self::with_runtime(initial_value, on_subscribed_change_fn_pin, SR::default())
	}

	pub(crate) fn with_runtime(
		initial_value: T,
		on_subscribed_change_fn_pin: HandlerFnPin,
		runtime: SR,
	) -> Self
	where
		T: Sized,
	{
		Self {
			signal: RawSignal::with_runtime(
				AssertSync((
					Mutex::new(on_subscribed_change_fn_pin),
					RwLock::new(initial_value),
				)),
				runtime,
			),
		}
	}

	pub(crate) fn read<'a>(self: Pin<&'a Self>) -> impl 'a + Borrow<T>
	where
		T: Sync,
	{
		let this = &self;
		ReactiveCellMutGuard(this.touch().read().unwrap())
	}

	pub(crate) fn read_exclusive<'a>(self: Pin<&'a Self>) -> impl 'a + Borrow<T> {
		let this = &self;
		ReactiveCellMutGuardExclusive(this.touch().write().unwrap())
	}

	fn touch(self: Pin<&Self>) -> &RwLock<T> {
		unsafe {
			// SAFETY: Doesn't defer memory access.
			&*(&self
				.project_ref()
				.signal
				.project_or_init::<E>(|_, slot| slot.write(()))
				.0
				 .0
				 .1 as *const _)
		}
	}
}

enum E {}
impl<
		T: ?Sized + Send,
		HandlerFnPin: Send + FnMut(&mut T, <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
		SR: SignalRuntimeRef,
	> Callbacks<AssertSync<(Mutex<HandlerFnPin>, RwLock<T>)>, (), SR> for E
{
	const UPDATE: Option<
		fn(
			eager: Pin<&AssertSync<(Mutex<HandlerFnPin>, RwLock<T>)>>,
			lazy: Pin<&()>,
		) -> isoprenoid::runtime::Update,
	> = None;

	const ON_SUBSCRIBED_CHANGE: Option<
		fn(
			signal: Pin<&RawSignal<AssertSync<(Mutex<HandlerFnPin>, RwLock<T>)>, (), SR>>,
			eager: Pin<&AssertSync<(Mutex<HandlerFnPin>, RwLock<T>)>>,
			lazy: Pin<&()>,
			subscribed: <<SR as SignalRuntimeRef>::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
		),
	> = {
		fn on_subscribed_change_fn_pin<
			T: ?Sized + Send,
			HandlerFnPin: Send + FnMut(&mut T, <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
			SR: SignalRuntimeRef,
		>(_: Pin<&RawSignal<AssertSync<(Mutex<HandlerFnPin>, RwLock<T>)>, (), SR>>,eager: Pin<&AssertSync<(Mutex<HandlerFnPin>, RwLock<T>)>>, _ :Pin<&()>, status: <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus){
			eager.0.0.lock().unwrap()(eager.0.1.write().unwrap().borrow_mut(), status)
		}

		Some(on_subscribed_change_fn_pin::<T,HandlerFnPin,SR>)
	};
}

impl<
		T: ?Sized + Send,
		HandlerFnPin: Send + FnMut(&mut T, <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
		SR: SignalRuntimeRef,
	> Source<SR> for ReactiveCellMut<T, HandlerFnPin, SR>
{
	type Output = T;

	fn touch(self: Pin<&Self>) {
		self.touch();
	}

	fn get_clone(self: Pin<&Self>) -> Self::Output
	where
		Self::Output: Sync + Clone,
	{
		self.read().borrow().clone()
	}

	fn get_clone_exclusive(self: Pin<&Self>) -> Self::Output
	where
		Self::Output: Clone,
	{
		self.touch().write().unwrap().clone()
	}

	fn read<'a>(self: Pin<&'a Self>) -> Box<dyn 'a + Borrow<Self::Output>>
	where
		Self::Output: Sync,
	{
		Box::new(self.read())
	}

	fn read_exclusive<'a>(self: Pin<&'a Self>) -> Box<dyn 'a + Borrow<Self::Output>> {
		Box::new(self.read_exclusive())
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
		HandlerFnPin: Send + FnMut(&mut T, <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
		SR: SignalRuntimeRef,
	> Subscribable<SR> for ReactiveCellMut<T, HandlerFnPin, SR>
{
	fn subscribe_inherently<'r>(self: Pin<&'r Self>) -> Option<Box<dyn 'r + Borrow<Self::Output>>> {
		//FIXME: This is inefficient.
		if self
			.project_ref()
			.signal
			.subscribe_inherently_or_init::<E>(|_, slot| slot.write(()))
			.is_some()
		{
			Some(Source::read_exclusive(self))
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
		HandlerFnPin: Send + FnMut(&mut T, <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
		SR: ?Sized + SignalRuntimeRef<Symbol: Sync>,
	> SourceCell<T, SR> for ReactiveCellMut<T, HandlerFnPin, SR>
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

	async fn update_async<U: Send>(&self, update: impl Send + FnOnce(&mut T) -> (U, Update)) -> U {
		self.signal
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
			.update_blocking(|value, _| update(&mut value.0 .1.write().unwrap()))
	}
}