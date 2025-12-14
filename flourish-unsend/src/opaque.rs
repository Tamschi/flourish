use std::{
	borrow::Borrow,
	future::Future,
	marker::{PhantomData, PhantomPinned},
	ops::Deref,
	pin::Pin,
	task::{Context, Poll},
};

use isoprenoid_unsend::runtime::{Propagation, SignalsRuntimeRef};

use crate::traits::{Guard, UnmanagedSignal, UnmanagedSignalCell};

pub enum Opaque {}

impl<T: ?Sized, SR: ?Sized + SignalsRuntimeRef> UnmanagedSignal<T, SR> for Opaque {
	fn touch(self: Pin<&Self>) {
		match *self {}
	}

	fn get_clone(self: Pin<&Self>) -> T
	where
		T: Clone,
	{
		match *self {}
	}

	fn read<'r>(self: Pin<&'r Self>) -> OpaqueGuard<T>
	where
		Self: Sized,
		T: 'r,
	{
		match *self {}
	}

	type Read<'r>
		= OpaqueGuard<T>
	where
		Self: 'r + Sized,
		T: 'r;

	fn read_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r,
	{
		match *self {}
	}

	fn subscribe(self: Pin<&Self>) {
		match *self {}
	}

	fn unsubscribe(self: Pin<&Self>) {
		match *self {}
	}

	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		match *self {}
	}
}

impl<T: ?Sized, SR: ?Sized + SignalsRuntimeRef> UnmanagedSignalCell<T, SR> for Opaque {
	fn change(self: Pin<&Self>, _: T)
	where
		T: 'static + Sized + PartialEq,
	{
		match *self {}
	}

	fn replace(self: Pin<&Self>, _: T)
	where
		T: 'static + Sized,
	{
		match *self {}
	}

	fn update(self: Pin<&Self>, _: impl 'static + FnOnce(&mut T) -> Propagation)
	where
		Self: Sized,
		T: 'static,
	{
		match *self {}
	}

	fn update_dyn(self: Pin<&Self>, _: Box<dyn 'static + FnOnce(&mut T) -> Propagation>)
	where
		T: 'static,
	{
		match *self {}
	}

	fn change_eager<'f>(self: Pin<&Self>, _: T) -> OpaqueFuture<Result<Result<T, T>, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized + PartialEq,
	{
		match *self {}
	}

	type ChangeEager<'f>
		= OpaqueFuture<Result<Result<T, T>, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	fn replace_eager<'f>(self: Pin<&Self>, _: T) -> OpaqueFuture<Result<T, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized,
	{
		match *self {}
	}

	type ReplaceEager<'f>
		= OpaqueFuture<Result<T, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	fn update_eager<'f, U: 'f, F: 'f + FnOnce(&mut T) -> (Propagation, U)>(
		self: Pin<&Self>,
		_: F,
	) -> OpaqueFuture<Result<U, F>>
	where
		Self: 'f + Sized,
	{
		match *self {}
	}

	type UpdateEager<'f, U: 'f, F: 'f>
		= OpaqueFuture<Result<U, F>>
	where
		Self: 'f + Sized;

	fn change_eager_dyn<'f>(
		self: Pin<&Self>,
		_: T,
	) -> Box<dyn 'f + Future<Output = Result<Result<T, T>, T>>>
	where
		T: 'f + Sized + PartialEq,
	{
		match *self {}
	}

	fn replace_eager_dyn<'f>(self: Pin<&Self>, _: T) -> Box<dyn 'f + Future<Output = Result<T, T>>>
	where
		T: 'f + Sized,
	{
		match *self {}
	}

	fn update_eager_dyn<'f>(
		self: Pin<&Self>,
		_: Box<dyn 'f + FnOnce(&mut T) -> Propagation>,
	) -> Box<dyn 'f + Future<Output = Result<(), Box<dyn 'f + FnOnce(&mut T) -> Propagation>>>>
	where
		T: 'f,
	{
		match *self {}
	}

	fn change_blocking(&self, _: T) -> Result<T, T>
	where
		T: Sized + PartialEq,
	{
		match *self {}
	}

	fn replace_blocking(&self, _: T) -> T
	where
		T: Sized,
	{
		match *self {}
	}

	fn update_blocking<U>(&self, _: impl FnOnce(&mut T) -> (Propagation, U)) -> U
	where
		Self: Sized,
	{
		match *self {}
	}

	fn update_blocking_dyn(&self, _: Box<dyn '_ + FnOnce(&mut T) -> Propagation>) {
		match *self {}
	}
}

pub struct OpaqueFuture<T> {
	_phantom: (PhantomData<T>, PhantomPinned),
	_vacant: Opaque,
}

impl<T> Future for OpaqueFuture<T> {
	type Output = T;

	fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
		match self._vacant {}
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
		match self._vacant {}
	}
}

impl<T: ?Sized> Borrow<T> for OpaqueGuard<T> {
	fn borrow(&self) -> &T {
		match self._vacant {}
	}
}
