use std::{
	borrow::Borrow,
	cell::{Ref, RefCell},
	fmt::{self, Debug, Formatter},
	future::Future,
	mem,
	ops::Deref,
	pin::Pin,
	sync::{Arc, Mutex},
};

use isoprenoid_unsend::{
	raw::{NoCallbacks, RawSignal},
	runtime::{Propagation, SignalsRuntimeRef},
};
use pin_project::pin_project;

use crate::{shadow_clone, traits::Guard};

use super::{UnmanagedSignal, UnmanagedSignalCell};

#[pin_project]
pub(crate) struct InertCell<T: ?Sized, SR: SignalsRuntimeRef> {
	#[pin]
	signal: RawSignal<RefCell<T>, (), SR>,
}

impl<T: ?Sized + Debug, SR: SignalsRuntimeRef + Debug> Debug for InertCell<T, SR>
where
	SR::Symbol: Debug,
{
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		f.debug_struct("InertCell")
			.field("signal", &&self.signal)
			.finish()
	}
}

pub(crate) struct InertCellGuard<'a, T: ?Sized>(Ref<'a, T>);

impl<'a, T: ?Sized> Guard<T> for InertCellGuard<'a, T> {}

impl<'a, T: ?Sized> Deref for InertCellGuard<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		self.0.deref()
	}
}

impl<'a, T: ?Sized> Borrow<T> for InertCellGuard<'a, T> {
	fn borrow(&self) -> &T {
		self.0.borrow()
	}
}

impl<T: ?Sized, SR: SignalsRuntimeRef> InertCell<T, SR> {
	pub(crate) fn with_runtime(initial_value: T, runtime: SR) -> Self
	where
		T: Sized,
	{
		Self {
			signal: RawSignal::with_runtime(RefCell::new(initial_value), runtime),
		}
	}

	pub(crate) fn read<'a>(self: Pin<&'a Self>) -> impl 'a + Guard<T> {
		InertCellGuard(self.touch().borrow())
	}

	fn touch(self: Pin<&Self>) -> &RefCell<T> {
		// SAFETY: Doesn't defer memory access.
		&*(&self
			.project_ref()
			.signal
			.project_or_init::<NoCallbacks>(|_, slot| slot.write(()))
			.0
			.get_ref())
	}
}

impl<T: ?Sized, SR: SignalsRuntimeRef> UnmanagedSignal<T, SR> for InertCell<T, SR> {
	fn touch(self: Pin<&Self>) {
		self.touch();
	}

	fn get_clone(self: Pin<&Self>) -> T
	where
		T: Clone,
	{
		self.read().clone()
	}

	fn read<'r>(self: Pin<&'r Self>) -> InertCellGuard<'r, T>
	where
		Self: Sized,
		T: 'r,
	{
		let touch = self.touch();
		InertCellGuard(touch.borrow())
	}

	type Read<'r>
		= InertCellGuard<'r, T>
	where
		Self: 'r + Sized,
		T: 'r;

	fn read_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r,
	{
		Box::new(self.read())
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
			.run_detached(|| signal.project_or_init::<NoCallbacks>(|_, slot| slot.write(())));
	}

	fn unsubscribe(self: Pin<&Self>) {
		self.project_ref().signal.unsubscribe()
	}
}

impl<T: ?Sized, SR: ?Sized + SignalsRuntimeRef> UnmanagedSignalCell<T, SR> for InertCell<T, SR> {
	fn change(self: Pin<&Self>, new_value: T)
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

	fn replace(self: Pin<&Self>, new_value: T)
	where
		T: 'static + Sized,
	{
		self.update(|value| {
			*value = new_value;
			Propagation::Propagate
		});
	}

	fn update(self: Pin<&Self>, update: impl 'static + FnOnce(&mut T) -> Propagation) {
		self.signal
			.clone_runtime_ref()
			.run_detached(|| self.touch());
		self.project_ref()
			.signal
			.update(|value, _| update(&mut *value.borrow_mut()))
	}

	fn update_dyn(self: Pin<&Self>, update: Box<dyn 'static + FnOnce(&mut T) -> Propagation>)
	where
		T: 'static,
	{
		self.signal
			.clone_runtime_ref()
			.run_detached(|| self.touch());
		self.project_ref()
			.signal
			.update(|value, _| update(&mut *value.borrow_mut()))
	}

	fn change_eager<'f>(
		self: Pin<&Self>,
		new_value: T,
	) -> private::DetachedFuture<'f, Result<Result<T, T>, T>>
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
					*r = Some(Ok(Ok(mem::replace(value, new_value))));
					(Propagation::Propagate, ())
				} else {
					*r = Some(Ok(Err(new_value)));
					(Propagation::Halt, ())
				}
			}
		});

		private::DetachedFuture(Box::pin(async move {
			//FIXME: Boxing seems to be currently required because of <https://github.com/rust-lang/rust/issues/100013>?
			use futures_lite::FutureExt;
			f.boxed_local().await.ok();
			Arc::try_unwrap(r)
				.map_err(|_| ())
				.expect("The `Arc`'s clone is dropped in the previous line.")
				.into_inner()
				.expect("unreachable")
				.expect("unreachable")
		}))
	}

	type ChangeEager<'f>
		= private::DetachedFuture<'f, Result<Result<T, T>, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	fn replace_eager<'f>(
		self: Pin<&Self>,
		new_value: T,
	) -> private::DetachedFuture<'f, Result<T, T>>
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

		private::DetachedFuture(Box::pin(async move {
			//FIXME: Boxing seems to be currently required because of <https://github.com/rust-lang/rust/issues/100013>?
			use futures_lite::FutureExt;
			f.boxed_local().await.ok();
			Arc::try_unwrap(r)
				.map_err(|_| ())
				.expect("The `Arc`'s clone is dropped in the previous line.")
				.into_inner()
				.expect("unreachable")
				.expect("unreachable")
		}))
	}

	type ReplaceEager<'f>
		= private::DetachedFuture<'f, Result<T, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	fn update_eager<'f, U: 'f, F: 'f + FnOnce(&mut T) -> (Propagation, U)>(
		self: Pin<&Self>,
		update: F,
	) -> private::DetachedFuture<'f, Result<U, F>>
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
				update(&mut *value.borrow_mut())
			}
		});
		private::DetachedFuture(Box::pin(async move {
			//FIXME: Boxing seems to be currently required because of <https://github.com/rust-lang/rust/issues/100013>?
			use futures_lite::FutureExt;
			f.boxed_local().await.map_err(|_| {
				Arc::try_unwrap(update)
					.map_err(|_| ())
					.expect("The `Arc`'s clone is dropped in the previous line.")
					.into_inner()
					.expect("unreachable")
					.expect("unreachable")
			})
		}))
	}

	type UpdateEager<'f, U: 'f, F: 'f>
		= private::DetachedFuture<'f, Result<U, F>>
	where
		Self: 'f + Sized;

	fn change_eager_dyn<'f>(
		self: Pin<&Self>,
		new_value: T,
	) -> Box<dyn 'f + Future<Output = Result<Result<T, T>, T>>>
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
						*r = Some(Ok(Ok(mem::replace(value, new_value))));
						Propagation::Propagate
					} else {
						*r = Some(Ok(Err(new_value)));
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
	) -> Box<dyn 'f + Future<Output = Result<T, T>>>
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

	fn update_eager_dyn<'f>(
		self: Pin<&Self>,
		update: Box<dyn 'f + FnOnce(&mut T) -> Propagation>,
	) -> Box<dyn 'f + Future<Output = Result<(), Box<dyn 'f + FnOnce(&mut T) -> Propagation>>>>
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
						update(&mut *value.borrow_mut())
					} else {
						Propagation::Halt
					},
					(),
				)
			}
		});
		Box::new(async move {
			f.await.map_err(|_| {
				Arc::into_inner(update)
					.expect("unreachable")
					.into_inner()
					.expect("unreachable")
					.expect("`Some`")
			})
		})
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
			.update_blocking(|value, _| update(&mut value.borrow_mut()))
	}

	fn update_blocking_dyn(&self, update: Box<dyn '_ + FnOnce(&mut T) -> Propagation>) {
		self.signal
			.update_blocking(|value, _| (update(&mut value.borrow_mut()), ()))
	}
}

/// Duplicated to avoid identities.
mod private {
	use std::{
		future::Future,
		pin::Pin,
		task::{Context, Poll},
	};

	use futures_lite::FutureExt;

	#[must_use = "Eager futures may still cancel their effect iff dropped."]
	pub(crate) struct DetachedFuture<'f, Output: 'f>(
		pub(super) Pin<Box<dyn 'f + Future<Output = Output>>>,
	);

	impl<'f, Output: 'f> Future for DetachedFuture<'f, Output> {
		type Output = Output;

		fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
			self.0.poll(cx)
		}
	}
}
