use std::{
	borrow::Borrow,
	future::Future,
	marker::PhantomData,
	ops::Deref,
	pin::Pin,
	task::{Context, Poll},
};

use flourish::{
	unmanaged::{Source, SourceCell, Subscribable},
	Guard, Propagation, SignalsRuntimeRef,
};

pub enum Opaque {}

impl<T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef> Source<T, SR> for Opaque {
	fn touch(self: Pin<&Self>) {
		const { unreachable!() }
	}

	fn get_clone(self: Pin<&Self>) -> T
	where
		T: Sync + Clone,
	{
		const { unreachable!() }
	}

	fn get_clone_exclusive(self: Pin<&Self>) -> T
	where
		T: Clone,
	{
		const { unreachable!() }
	}

	fn read<'r>(self: Pin<&'r Self>) -> OpaqueGuard<T>
	where
		Self: Sized,
		T: 'r + Sync,
	{
		const { unreachable!() }
	}

	type Read<'r> = OpaqueGuard<T>
	where
		Self: 'r + Sized,
		T: 'r + Sync;

	fn read_exclusive<'r>(self: Pin<&'r Self>) -> OpaqueGuard<T>
	where
		Self: Sized,
		T: 'r,
	{
		const { unreachable!() }
	}

	type ReadExclusive<'r> = OpaqueGuard<T>
	where
		Self: 'r + Sized, T: 'r;

	fn read_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r + Sync,
	{
		const { unreachable!() }
	}

	fn read_exclusive_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r,
	{
		const { unreachable!() }
	}

	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		const { unreachable!() }
	}
}

impl<T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef> Subscribable<T, SR> for Opaque {
	fn subscribe(self: Pin<&Self>) {
		const { unreachable!() }
	}

	fn unsubscribe(self: Pin<&Self>) {
		const { unreachable!() }
	}
}

impl<T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef> SourceCell<T, SR> for Opaque {
	fn change(self: Pin<&Self>, _: T)
	where
		T: 'static + Sized + PartialEq,
	{
		const { unreachable!() }
	}

	fn replace(self: Pin<&Self>, _: T)
	where
		T: 'static + Sized,
	{
		const { unreachable!() }
	}

	fn update(self: Pin<&Self>, _: impl 'static + Send + FnOnce(&mut T) -> Propagation)
	where
		Self: Sized,
		T: 'static,
	{
		const { unreachable!() }
	}

	fn update_dyn(self: Pin<&Self>, _: Box<dyn 'static + Send + FnOnce(&mut T) -> Propagation>)
	where
		T: 'static,
	{
		const { unreachable!() }
	}

	fn change_eager<'f>(self: Pin<&Self>, _: T) -> OpaqueFuture<Result<Result<T, T>, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized + PartialEq,
	{
		const { unreachable!() }
	}

	type ChangeEager<'f> = OpaqueFuture<Result<Result<T, T>, T>>
		    where
			    Self: 'f + Sized,
			    T: 'f + Sized;

	fn replace_eager<'f>(self: Pin<&Self>, _: T) -> OpaqueFuture<Result<T, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized,
	{
		const { unreachable!() }
	}

	type ReplaceEager<'f> = OpaqueFuture<Result<T, T>>
		    where
			    Self: 'f + Sized,
			    T: 'f + Sized;

	fn update_eager<'f, U: 'f + Send, F: 'f + Send + FnOnce(&mut T) -> (Propagation, U)>(
		self: Pin<&Self>,
		_: F,
	) -> OpaqueFuture<Result<U, F>>
	where
		Self: 'f + Sized,
	{
		const { unreachable!() }
	}

	type UpdateEager<'f, U: 'f, F: 'f> = OpaqueFuture<Result<U, F>>
		    where
			    Self: 'f + Sized;

	fn change_eager_dyn<'f>(
		self: Pin<&Self>,
		_: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<Result<T, T>, T>>>
	where
		T: 'f + Sized + PartialEq,
	{
		const { unreachable!() }
	}

	fn replace_eager_dyn<'f>(
		self: Pin<&Self>,
		_: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<T, T>>>
	where
		T: 'f + Sized,
	{
		const { unreachable!() }
	}

	fn update_eager_dyn<'f>(
		self: Pin<&Self>,
		_: Box<dyn 'f + Send + FnOnce(&mut T) -> Propagation>,
	) -> Box<
		dyn 'f
			+ Send
			+ Future<Output = Result<(), Box<dyn 'f + Send + FnOnce(&mut T) -> Propagation>>>,
	>
	where
		T: 'f,
	{
		const { unreachable!() }
	}

	fn change_blocking(&self, _: T) -> Result<T, T>
	where
		T: Sized + PartialEq,
	{
		const { unreachable!() }
	}

	fn replace_blocking(&self, _: T) -> T
	where
		T: Sized,
	{
		const { unreachable!() }
	}

	fn update_blocking<U>(&self, _: impl FnOnce(&mut T) -> (Propagation, U)) -> U
	where
		Self: Sized,
	{
		const { unreachable!() }
	}

	fn update_blocking_dyn(&self, _: Box<dyn '_ + FnOnce(&mut T) -> Propagation>) {
		const { unreachable!() }
	}
}

pub struct OpaqueFuture<T> {
	pub(crate) _phantom: PhantomData<T>,
	pub(crate) _vacant: Opaque,
}

/// # Safety
///
/// `OpaqueFuture` is vacant.
unsafe impl<T> Send for OpaqueFuture<T> {}

impl<T> Future for OpaqueFuture<T> {
	type Output = T;

	fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
		const { unreachable!() }
	}
}

pub struct OpaqueGuard<T: ?Sized> {
	pub(crate) _phantom: PhantomData<T>,
	pub(crate) _vacant: Opaque,
}

impl<T: ?Sized> Guard<T> for OpaqueGuard<T> {}

impl<T: ?Sized> Deref for OpaqueGuard<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		const { unreachable!() }
	}
}

impl<T: ?Sized> Borrow<T> for OpaqueGuard<T> {
	fn borrow(&self) -> &T {
		const { unreachable!() }
	}
}
