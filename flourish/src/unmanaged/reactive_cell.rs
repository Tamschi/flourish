use std::{
	fmt::{self, Debug, Formatter},
	future::Future,
	mem,
	pin::Pin,
	sync::{Arc, Mutex, RwLock},
};

use isoprenoid::{
	raw::{Callbacks, RawSignal},
	runtime::{CallbackTableTypes, Propagation, SignalsRuntimeRef},
};
use pin_project::pin_project;

use crate::{
	shadow_clone,
	traits::{Guard, ReadGuard, WriteGuard},
	MaybeReplaced, MaybeSet,
};

use super::{UnmanagedSignal, UnmanagedSignalCell};

#[pin_project]
#[repr(transparent)]
pub(crate) struct ReactiveCell<
	T: ?Sized + Send,
	HandlerFnPin: Send
		+ FnMut(&T, <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus) -> Propagation,
	SR: SignalsRuntimeRef,
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
		SR: SignalsRuntimeRef + Debug,
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

// TODO: Safety documentation.
unsafe impl<
		T: ?Sized + Send,
		HandlerFnPin: Send
			+ FnMut(
				&T,
				<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
			) -> Propagation,
		SR: SignalsRuntimeRef + Sync,
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

impl<
		T: ?Sized + Send,
		HandlerFnPin: Send
			+ FnMut(
				&T,
				<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
			) -> Propagation,
		SR: SignalsRuntimeRef,
	> ReactiveCell<T, HandlerFnPin, SR>
{
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

	pub(crate) fn read<'a>(self: Pin<&'a Self>) -> ReadGuard<'a, T>
	where
		T: Sync,
	{
		ReadGuard(self.touch().read().unwrap())
	}

	pub(crate) fn read_exclusive<'a>(self: Pin<&'a Self>) -> WriteGuard<'a, T> {
		WriteGuard(self.touch().write().unwrap())
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
		SR: SignalsRuntimeRef,
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
			subscribed: <<SR as SignalsRuntimeRef>::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
		) -> Propagation,
	> = {
		fn on_subscribed_change_fn_pin<
			T: ?Sized + Send,
			HandlerFnPin: Send + FnMut(&T, <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus) -> Propagation,
			SR: SignalsRuntimeRef,
		>(_: Pin<&RawSignal<AssertSync<(Mutex<HandlerFnPin>, RwLock<T>)>, (), SR>>, eager: Pin<&AssertSync<(Mutex<HandlerFnPin>, RwLock<T>)>>, _ :Pin<&()>, status: <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus) -> Propagation{
			eager.0.0.lock().unwrap()(&*eager.0.1.read().unwrap(), status)
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
		SR: SignalsRuntimeRef,
	> UnmanagedSignal<T, SR> for ReactiveCell<T, HandlerFnPin, SR>
{
	fn touch(self: Pin<&Self>) {
		self.touch();
	}

	fn get_clone(self: Pin<&Self>) -> T
	where
		T: Sync + Clone,
	{
		self.read().clone()
	}

	fn get_clone_exclusive(self: Pin<&Self>) -> T
	where
		T: Clone,
	{
		self.read_exclusive().clone()
	}

	fn read<'r>(self: Pin<&'r Self>) -> impl 'r + Guard<T>
	where
		Self: Sized,
		T: 'r + Sync,
	{
		self.read()
	}

	fn read_exclusive<'r>(self: Pin<&'r Self>) -> impl 'r + Guard<T>
	where
		Self: Sized,
		T: 'r,
	{
		self.read_exclusive()
	}

	fn read_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r + Sync,
	{
		Box::new(self.read())
	}

	fn read_exclusive_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r,
	{
		Box::new(self.read_exclusive())
	}

	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		self.signal.clone_runtime_ref()
	}

	fn subscribe(self: Pin<&Self>) {
		let signal = self.project_ref().signal;
		signal.subscribe();
		signal
			.clone_runtime_ref()
			.run_detached(|| signal.project_or_init::<E>(|_, slot| slot.write(())));
	}

	fn unsubscribe(self: Pin<&Self>) {
		self.project_ref().signal.unsubscribe()
	}
}

impl<
		T: ?Sized + Send,
		HandlerFnPin: Send
			+ FnMut(
				&T,
				<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
			) -> Propagation,
		SR: ?Sized + SignalsRuntimeRef,
	> UnmanagedSignalCell<T, SR> for ReactiveCell<T, HandlerFnPin, SR>
{
	fn set(self: Pin<&Self>, new_value: T)
	where
		T: 'static + Sized,
	{
		self.update(|value| {
			*value = new_value;
			Propagation::Propagate
		});
	}

	fn set_distinct(self: Pin<&Self>, new_value: T)
	where
		T: 'static + Sized + PartialEq,
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

	fn update(self: Pin<&Self>, update: impl 'static + Send + FnOnce(&mut T) -> Propagation) {
		self.signal
			.clone_runtime_ref()
			.run_detached(|| self.touch());
		self.project_ref()
			.signal
			.update(|value, _| update(&mut value.0 .1.write().unwrap()))
	}

	fn update_dyn(self: Pin<&Self>, update: Box<dyn 'static + Send + FnOnce(&mut T) -> Propagation>)
	where
		T: 'static,
	{
		self.signal
			.clone_runtime_ref()
			.run_detached(|| self.touch());
		self.project_ref()
			.signal
			.update(|value, _| update(&mut value.0 .1.write().unwrap()))
	}

	fn set_eager<'f>(
		self: Pin<&Self>,
		new_value: T,
	) -> impl use<'f, T, HandlerFnPin, SR> + 'f + Send + Future<Output = Result<(), T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized,
	{
		let r = Arc::new(Mutex::new(Some(Err(new_value))));
		let f = self.update_eager({
			let r = Arc::downgrade(&r);
			move |value| {
				let Some(r) = r.upgrade() else {
					return (Propagation::Halt, ());
				};
				let mut r = r.try_lock().unwrap();
				let new_value = r.take().unwrap().map(|_| ()).unwrap_err();
				*r = Some(Ok(*value = new_value));
				(Propagation::Propagate, ())
			}
		});

		async move {
			f.await.ok();
			Arc::try_unwrap(r)
				.map_err(|_| ())
				.expect("The `Arc`'s clone is dropped in the previous line.")
				.into_inner()
				.expect("unreachable")
				.expect("unreachable")
		}
	}

	fn set_distinct_eager<'f>(
		self: Pin<&Self>,
		new_value: T,
	) -> impl use<'f, T, HandlerFnPin, SR> + 'f + Send + Future<Output = Result<MaybeSet<T>, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized + Eq,
	{
		let r = Arc::new(Mutex::new(Some(Err(new_value))));
		let f = self.update_eager({
			let r = Arc::downgrade(&r);
			move |value| {
				let Some(r) = r.upgrade() else {
					return (Propagation::Halt, ());
				};
				let mut r = r.try_lock().unwrap();
				let new_value = r.take().unwrap().map(|_| ()).unwrap_err();
				if *value != new_value {
					*value = new_value;
					*r = Some(Ok(MaybeSet::Set));
					(Propagation::Propagate, ())
				} else {
					*r = Some(Ok(MaybeSet::Unchanged(new_value)));
					(Propagation::Halt, ())
				}
			}
		});

		async move {
			f.await.ok();
			Arc::try_unwrap(r)
				.map_err(|_| ())
				.expect("The `Arc`'s clone is dropped in the previous line.")
				.into_inner()
				.expect("unreachable")
				.expect("unreachable")
		}
	}

	fn replace_eager<'f>(
		self: Pin<&Self>,
		new_value: T,
	) -> impl use<'f, T, HandlerFnPin, SR> + 'f + Send + Future<Output = Result<T, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized,
	{
		let r = Arc::new(Mutex::new(Some(Err(new_value))));
		let f = self.update_eager({
			let r = Arc::downgrade(&r);
			move |value| {
				let Some(r) = r.upgrade() else {
					return (Propagation::Halt, ());
				};
				let mut r = r.try_lock().unwrap();
				let new_value = r.take().unwrap().map(|_| ()).unwrap_err();
				*r = Some(Ok(mem::replace(value, new_value)));
				(Propagation::Propagate, ())
			}
		});

		async move {
			f.await.ok();
			Arc::try_unwrap(r)
				.map_err(|_| ())
				.expect("The `Arc`'s clone is dropped in the previous line.")
				.into_inner()
				.expect("unreachable")
				.expect("unreachable")
		}
	}

	fn replace_distinct_eager<'f>(
		self: Pin<&Self>,
		new_value: T,
	) -> impl use<'f, T, HandlerFnPin, SR> + 'f + Send + Future<Output = Result<MaybeReplaced<T>, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized + PartialEq,
	{
		let r = Arc::new(Mutex::new(Some(Err(new_value))));
		let f = self.update_eager({
			let r = Arc::downgrade(&r);
			move |value| {
				let Some(r) = r.upgrade() else {
					return (Propagation::Halt, ());
				};
				let mut r = r.try_lock().unwrap();
				let new_value = r.take().unwrap().map(|_| ()).unwrap_err();
				if *value != new_value {
					*r = Some(Ok(MaybeReplaced::Replaced(mem::replace(value, new_value))));
					(Propagation::Propagate, ())
				} else {
					*r = Some(Ok(MaybeReplaced::Unchanged(new_value)));
					(Propagation::Halt, ())
				}
			}
		});

		async move {
			f.await.ok();
			Arc::try_unwrap(r)
				.map_err(|_| ())
				.expect("The `Arc`'s clone is dropped in the previous line.")
				.into_inner()
				.expect("unreachable")
				.expect("unreachable")
		}
	}

	fn update_eager<'f, U: 'f + Send, F: 'f + Send + FnOnce(&mut T) -> (Propagation, U)>(
		self: Pin<&Self>,
		update: F,
	) -> impl use<'f, T, HandlerFnPin, SR, U, F> + 'f + Send + Future<Output = Result<U, F>>
	where
		Self: 'f + Sized,
	{
		let update = Arc::new(Mutex::new(Some(update)));
		let f = self.project_ref().signal.update_eager_pin({
			shadow_clone!(update);
			move |value, _| {
				let update = update
					.try_lock()
					.expect("unreachable")
					.take()
					.expect("unreachable");
				update(&mut value.0 .1.write().unwrap())
			}
		});
		async move {
			f.await.map_err(|_| {
				Arc::try_unwrap(update)
					.map_err(|_| ())
					.expect("The `Arc`'s clone is dropped in the previous line.")
					.into_inner()
					.expect("unreachable")
					.expect("unreachable")
			})
		}
	}

	fn set_eager_dyn<'f>(
		self: Pin<&Self>,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<(), T>>>
	where
		T: 'f + Sized,
	{
		let r = Arc::new(Mutex::new(Some(Err(new_value))));
		let f: Pin<Box<_>> = self
			.update_eager_dyn({
				let r = Arc::downgrade(&r);
				Box::new(move |value: &mut T| {
					let Some(r) = r.upgrade() else {
						return Propagation::Halt;
					};
					let mut r = r.try_lock().unwrap();
					let new_value = r.take().unwrap().map(|_| ()).unwrap_err();
					*r = Some(Ok(*value = new_value));
					Propagation::Propagate
				})
			})
			.into();

		Box::new(async move {
			f.await.ok();
			Arc::try_unwrap(r)
				.map_err(|_| ())
				.expect("The `Arc`'s clone is dropped in the previous line.")
				.into_inner()
				.expect("unreachable")
				.expect("unreachable")
		})
	}

	fn set_distinct_eager_dyn<'f>(
		self: Pin<&Self>,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<MaybeSet<T>, T>>>
	where
		T: 'f + Sized + Eq,
	{
		let r = Arc::new(Mutex::new(Some(Err(new_value))));
		let f: Pin<Box<_>> = self
			.update_eager_dyn({
				let r = Arc::downgrade(&r);
				Box::new(move |value: &mut T| {
					let Some(r) = r.upgrade() else {
						return Propagation::Halt;
					};
					let mut r = r.try_lock().unwrap();
					let new_value = r.take().unwrap().map(|_| ()).unwrap_err();
					if *value != new_value {
						*value = new_value;
						*r = Some(Ok(MaybeSet::Set));
						Propagation::Propagate
					} else {
						*r = Some(Ok(MaybeSet::Unchanged(new_value)));
						Propagation::Halt
					}
				})
			})
			.into();

		Box::new(async move {
			f.await.ok();
			Arc::try_unwrap(r)
				.map_err(|_| ())
				.expect("The `Arc`'s clone is dropped in the previous line.")
				.into_inner()
				.expect("unreachable")
				.expect("unreachable")
		})
	}

	fn replace_eager_dyn<'f>(
		self: Pin<&Self>,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<T, T>>>
	where
		T: 'f + Sized,
	{
		let r = Arc::new(Mutex::new(Some(Err(new_value))));
		let f: Pin<Box<_>> = self
			.update_eager_dyn({
				let r = Arc::downgrade(&r);
				Box::new(move |value: &mut T| {
					let Some(r) = r.upgrade() else {
						return Propagation::Halt;
					};
					let mut r = r.try_lock().unwrap();
					let new_value = r.take().unwrap().map(|_| ()).unwrap_err();
					*r = Some(Ok(mem::replace(value, new_value)));
					Propagation::Propagate
				})
			})
			.into();

		Box::new(async move {
			f.await.ok();
			Arc::try_unwrap(r)
				.map_err(|_| ())
				.expect("The `Arc`'s clone is dropped in the previous line.")
				.into_inner()
				.expect("unreachable")
				.expect("unreachable")
		})
	}

	fn replace_distinct_eager_dyn<'f>(
		self: Pin<&Self>,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<MaybeReplaced<T>, T>>>
	where
		T: 'f + Sized + PartialEq,
	{
		let r = Arc::new(Mutex::new(Some(Err(new_value))));
		let f: Pin<Box<_>> = self
			.update_eager_dyn({
				let r = Arc::downgrade(&r);
				Box::new(move |value: &mut T| {
					let Some(r) = r.upgrade() else {
						return Propagation::Halt;
					};
					let mut r = r.try_lock().unwrap();
					let new_value = r.take().unwrap().map(|_| ()).unwrap_err();
					if *value != new_value {
						*r = Some(Ok(MaybeReplaced::Replaced(mem::replace(value, new_value))));
						Propagation::Propagate
					} else {
						*r = Some(Ok(MaybeReplaced::Unchanged(new_value)));
						Propagation::Halt
					}
				})
			})
			.into();

		Box::new(async move {
			f.await.ok();
			Arc::try_unwrap(r)
				.map_err(|_| ())
				.expect("The `Arc`'s clone is dropped in the previous line.")
				.into_inner()
				.expect("unreachable")
				.expect("unreachable")
		})
	}

	fn update_eager_dyn<'f>(
		self: Pin<&Self>,
		update: Box<dyn 'f + Send + FnOnce(&mut T) -> Propagation>,
	) -> Box<
		dyn 'f
			+ Send
			+ Future<Output = Result<(), Box<dyn 'f + Send + FnOnce(&mut T) -> Propagation>>>,
	>
	where
		T: 'f,
	{
		let update = Arc::new(Mutex::new(Some(update)));
		let f = self.project_ref().signal.update_eager_pin({
			let update = Arc::downgrade(&update);
			move |value, _| {
				(
					if let Some(update) = update.upgrade() {
						let update = update
							.try_lock()
							.expect("unreachable")
							.take()
							.expect("unreachable");
						update(&mut *value.0 .1.write().unwrap())
					} else {
						Propagation::Halt
					},
					(),
				)
			}
		});
		let f: Box<
			dyn Send
				+ Future<Output = Result<(), Box<dyn 'f + Send + FnOnce(&mut T) -> Propagation>>>,
		> = Box::new(async move {
			f.await.map_err(|_| {
				Arc::into_inner(update)
					.expect("unreachable")
					.into_inner()
					.expect("unreachable")
					.expect("`Some`")
			})
		});
		unsafe {
			//SAFETY: Lifetime extension. The closure cannot be called after `*self` is
			//        dropped, because dropping the `RawSignal` implicitly purges the ID.
			mem::transmute::<
				Box<
					dyn Send
						+ Future<
							Output = Result<(), Box<dyn 'f + Send + FnOnce(&mut T) -> Propagation>>,
						>,
				>,
				Box<
					dyn Send
						+ Future<
							Output = Result<(), Box<dyn 'f + Send + FnOnce(&mut T) -> Propagation>>,
						>,
				>,
			>(f)
		}
	}

	fn set_blocking(&self, new_value: T)
	where
		T: Sized,
	{
		self.update_blocking(|value| (Propagation::Propagate, *value = new_value))
	}

	fn set_distinct_blocking(&self, new_value: T) -> MaybeSet<T>
	where
		T: Sized + Eq,
	{
		self.update_blocking(|value| {
			if *value != new_value {
				*value = new_value;
				(Propagation::Propagate, MaybeSet::Set)
			} else {
				(Propagation::Halt, MaybeSet::Unchanged(new_value))
			}
		})
	}

	fn replace_blocking(&self, new_value: T) -> T
	where
		T: Sized,
	{
		self.update_blocking(|value| (Propagation::Propagate, mem::replace(value, new_value)))
	}

	fn replace_distinct_blocking(&self, new_value: T) -> MaybeReplaced<T>
	where
		T: Sized + PartialEq,
	{
		self.update_blocking(|value| {
			if *value != new_value {
				(
					Propagation::Propagate,
					MaybeReplaced::Replaced(mem::replace(value, new_value)),
				)
			} else {
				(Propagation::Halt, MaybeReplaced::Unchanged(new_value))
			}
		})
	}

	fn update_blocking<U>(&self, update: impl FnOnce(&mut T) -> (Propagation, U)) -> U {
		self.signal
			.update_blocking(|value, _| update(&mut value.0 .1.write().unwrap()))
	}

	fn update_blocking_dyn(&self, update: Box<dyn '_ + FnOnce(&mut T) -> Propagation>) {
		self.signal
			.update_blocking(|value, _| (update(&mut value.0 .1.write().unwrap()), ()))
	}
}
