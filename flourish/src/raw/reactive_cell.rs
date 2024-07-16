use std::{
	borrow::Borrow,
	fmt::{self, Debug, Formatter},
	mem,
	pin::Pin,
	sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use isoprenoid::{
	raw::{Callbacks, RawSignal},
	runtime::{CallbackTableTypes, Propagation, SignalRuntimeRef},
};
use pin_project::pin_project;

use super::{Source, SourceCell, Subscribable};

#[pin_project]
#[repr(transparent)]
pub struct ReactiveCell<
	T: ?Sized + Send,
	HandlerFnPin: Send
		+ FnMut(&T, <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus) -> Propagation,
	SR: SignalRuntimeRef,
> {
	#[pin]
	signal: RawSignal<AssertSync<(Mutex<HandlerFnPin>, RwLock<T>)>, (), SR>,
}

impl<
		T: ?Sized + Send + Debug,
		HandlerFnPin: Send
			+ FnMut(
				&T,
				<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
			) -> Propagation
			+ Debug,
		SR: SignalRuntimeRef + Debug,
	> Debug for ReactiveCell<T, HandlerFnPin, SR>
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
		HandlerFnPin: Send
			+ FnMut(
				&T,
				<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
			) -> Propagation,
		SR: SignalRuntimeRef + Sync,
	> Sync for ReactiveCell<T, HandlerFnPin, SR>
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
		HandlerFnPin: Send
			+ FnMut(
				&T,
				<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
			) -> Propagation,
		SR: SignalRuntimeRef,
	> ReactiveCell<T, HandlerFnPin, SR>
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
		ReactiveCellGuard(this.touch().read().unwrap())
	}

	pub(crate) fn read_exclusive<'a>(self: Pin<&'a Self>) -> impl 'a + Borrow<T> {
		let this = &self;
		ReactiveCellGuardExclusive(this.touch().write().unwrap())
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
		HandlerFnPin: Send
			+ FnMut(
				&T,
				<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
			) -> Propagation,
		SR: SignalRuntimeRef,
	> Callbacks<AssertSync<(Mutex<HandlerFnPin>, RwLock<T>)>, (), SR> for E
{
	const UPDATE: Option<
		fn(
			eager: Pin<&AssertSync<(Mutex<HandlerFnPin>, RwLock<T>)>>,
			lazy: Pin<&()>,
		) -> isoprenoid::runtime::Propagation,
	> = None;

	const ON_SUBSCRIBED_CHANGE: Option<
		fn(
			signal: Pin<&RawSignal<AssertSync<(Mutex<HandlerFnPin>, RwLock<T>)>, (), SR>>,
			eager: Pin<&AssertSync<(Mutex<HandlerFnPin>, RwLock<T>)>>,
			lazy: Pin<&()>,
			subscribed: <<SR as SignalRuntimeRef>::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
		) -> Propagation,
	> = {
		fn on_subscribed_change_fn_pin<
			T: ?Sized + Send,
			HandlerFnPin: Send + FnMut(&T, <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus) -> Propagation,
			SR: SignalRuntimeRef,
		>(_: Pin<&RawSignal<AssertSync<(Mutex<HandlerFnPin>, RwLock<T>)>, (), SR>>, eager: Pin<&AssertSync<(Mutex<HandlerFnPin>, RwLock<T>)>>, _ :Pin<&()>, status: <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus) -> Propagation{
			eager.0.0.lock().unwrap()(eager.0.1.read().unwrap().borrow(), status)
		}

		Some(on_subscribed_change_fn_pin::<T,HandlerFnPin,SR>)
	};
}

impl<
		T: ?Sized + Send,
		HandlerFnPin: Send
			+ FnMut(
				&T,
				<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
			) -> Propagation,
		SR: SignalRuntimeRef,
	> Source<SR> for ReactiveCell<T, HandlerFnPin, SR>
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
		HandlerFnPin: Send
			+ FnMut(
				&T,
				<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
			) -> Propagation,
		SR: SignalRuntimeRef,
	> Subscribable<SR> for ReactiveCell<T, HandlerFnPin, SR>
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
		HandlerFnPin: Send
			+ FnMut(
				&T,
				<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
			) -> Propagation,
		SR: ?Sized + SignalRuntimeRef<Symbol: Sync>,
	> SourceCell<T, SR> for ReactiveCell<T, HandlerFnPin, SR>
{
	fn change(self: Pin<&Self>, new_value: T)
	where
		T: 'static + Sized + PartialEq,
		SR::Symbol: Sync,
	{
		self.update(|value| {
			if *value != new_value {
				*value = new_value;
				Propagation::Propagate
			} else {
				Propagation::Halt
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
			Propagation::Propagate
		});
	}

	fn update(self: Pin<&Self>, update: impl 'static + Send + FnOnce(&mut T) -> Propagation) {
		self.signal
			.clone_runtime_ref()
			.run_detached(|| self.touch());
		self.project_ref()
			.signal
			.update(|value, _| update(&mut value.0 .1.write().unwrap()))
	}

	async fn update_async<U: Send>(
		&self,
		update: impl Send + FnOnce(&mut T) -> (Propagation, U),
	) -> U {
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
				(Propagation::Propagate, Ok(mem::replace(value, new_value)))
			} else {
				(Propagation::Halt, Err(new_value))
			}
		})
	}

	fn replace_blocking(&self, new_value: T) -> T
	where
		T: Sized,
	{
		self.update_blocking(|value| (Propagation::Propagate, mem::replace(value, new_value)))
	}

	fn update_blocking<U>(&self, update: impl FnOnce(&mut T) -> (Propagation, U)) -> U {
		self.signal
			.update_blocking(|value, _| update(&mut value.0 .1.write().unwrap()))
	}
}
