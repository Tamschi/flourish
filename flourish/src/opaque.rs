use std::{
	borrow::Borrow,
	future::Future,
	marker::PhantomData,
	ops::Deref,
	pin::Pin,
	task::{Context, Poll},
};

use isoprenoid::runtime::SignalsRuntimeRef;

use crate::traits::{Guard, Subscribable, UnmanagedSignal, UnmanagedSignalCell};

pub enum Opaque {}

impl<T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef> UnmanagedSignal<T, SR> for Opaque {
	fn touch(self: Pin<&Self>) {
		match *self {}
	}

	fn get_clone(self: Pin<&Self>) -> T
	where
		T: Sync + Clone,
	{
		match *self {}
	}

	fn get_clone_exclusive(self: Pin<&Self>) -> T
	where
		T: Clone,
	{
		match *self {}
	}

	fn read<'r>(self: Pin<&'r Self>) -> OpaqueGuard<T>
	where
		Self: Sized,
		T: 'r + Sync,
	{
		match *self {}
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
		match *self {}
	}

	type ReadExclusive<'r> = OpaqueGuard<T>
	where
		Self: 'r + Sized, T: 'r;

	fn read_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r + Sync,
	{
		match *self {}
	}

	fn read_exclusive_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Guard<T>>
	where
		T: 'r,
	{
		match *self {}
	}

	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		match *self {}
	}
}

impl<T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef> Subscribable<T, SR> for Opaque {
	fn subscribe(self: Pin<&Self>) {
		match *self {}
	}

	fn unsubscribe(self: Pin<&Self>) {
		match *self {}
	}
}

impl<T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef> UnmanagedSignalCell<T, SR> for Opaque {
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

	fn update(
		self: Pin<&Self>,
		_: impl 'static + Send + FnOnce(&mut T) -> isoprenoid::runtime::Propagation,
	) where
		Self: Sized,
		T: 'static,
	{
		match *self {}
	}

	fn update_dyn(
		self: Pin<&Self>,
		_: Box<dyn 'static + Send + FnOnce(&mut T) -> isoprenoid::runtime::Propagation>,
	) where
		T: 'static,
	{
		match *self {}
	}

	fn change_async<'f>(self: Pin<&Self>, new_value: T) -> Self::ChangeAsync<'f>
	where
		Self: 'f + Sized,
		T: 'f + Sized + PartialEq,
	{
		match *self {}
	}

	type ChangeAsync<'f> = OpaqueAsyncFuture<Result<Result<T, T>, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	fn replace_async<'f>(self: Pin<&Self>, new_value: T) -> Self::ReplaceAsync<'f>
	where
		Self: 'f + Sized,
		T: 'f + Sized,
	{
		match *self {}
	}

	type ReplaceAsync<'f> = OpaqueAsyncFuture<Result<T, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized;

	fn update_async<
		'f,
		U: 'f + Send,
		F: 'f + Send + FnOnce(&mut T) -> (isoprenoid::runtime::Propagation, U),
	>(
		self: Pin<&Self>,
		update: F,
	) -> Self::UpdateAsync<'f, U, F>
	where
		Self: 'f + Sized,
	{
		match *self {}
	}

	type UpdateAsync<'f, U: 'f, F: 'f> = OpaqueAsyncFuture<Result<U, F>>
	where
		Self: 'f + Sized;

	fn change_async_dyn<'f>(
		self: Pin<&Self>,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<Result<T, T>, T>>>
	where
		T: 'f + Sized + PartialEq,
	{
		match *self {}
	}

	fn replace_async_dyn<'f>(
		self: Pin<&Self>,
		new_value: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<T, T>>>
	where
		T: 'f + Sized,
	{
		match *self {}
	}

	fn update_async_dyn<'f>(
		self: Pin<&Self>,
		update: Box<dyn 'f + Send + FnOnce(&mut T) -> isoprenoid::runtime::Propagation>,
	) -> Box<
		dyn 'f
			+ Send
			+ Future<
				Output = Result<
					(),
					Box<dyn 'f + Send + FnOnce(&mut T) -> isoprenoid::runtime::Propagation>,
				>,
			>,
	>
	where
		T: 'f,
	{
		match *self {}
	}

	fn change_eager<'f>(self: Pin<&Self>, _: T) -> OpaqueEagerFuture<Result<Result<T, T>, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized + PartialEq,
	{
		match *self {}
	}

	type ChangeEager<'f> = OpaqueEagerFuture<Result<Result<T, T>, T>>
		    where
			    Self: 'f + Sized,
			    T: 'f + Sized;

	fn replace_eager<'f>(self: Pin<&Self>, _: T) -> OpaqueEagerFuture<Result<T, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized,
	{
		match *self {}
	}

	type ReplaceEager<'f> = OpaqueEagerFuture<Result<T, T>>
		    where
			    Self: 'f + Sized,
			    T: 'f + Sized;

	fn update_eager<
		'f,
		U: 'f + Send,
		F: 'f + Send + FnOnce(&mut T) -> (isoprenoid::runtime::Propagation, U),
	>(
		self: Pin<&Self>,
		_: F,
	) -> OpaqueEagerFuture<Result<U, F>>
	where
		Self: 'f + Sized,
	{
		match *self {}
	}

	type UpdateEager<'f, U: 'f, F: 'f> = OpaqueEagerFuture<Result<U, F>>
		    where
			    Self: 'f + Sized;

	fn change_eager_dyn<'f>(
		self: Pin<&Self>,
		_: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<Result<T, T>, T>>>
	where
		T: 'f + Sized + PartialEq,
	{
		match *self {}
	}

	fn replace_eager_dyn<'f>(
		self: Pin<&Self>,
		_: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<T, T>>>
	where
		T: 'f + Sized,
	{
		match *self {}
	}

	fn update_eager_dyn<'f>(
		self: Pin<&Self>,
		_: Box<dyn 'f + Send + FnOnce(&mut T) -> isoprenoid::runtime::Propagation>,
	) -> Box<
		dyn 'f
			+ Send
			+ Future<
				Output = Result<
					(),
					Box<dyn 'f + Send + FnOnce(&mut T) -> isoprenoid::runtime::Propagation>,
				>,
			>,
	>
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

	fn update_blocking<U>(
		&self,
		_: impl FnOnce(&mut T) -> (isoprenoid::runtime::Propagation, U),
	) -> U
	where
		Self: Sized,
	{
		match *self {}
	}

	fn update_blocking_dyn(
		&self,
		_: Box<dyn '_ + FnOnce(&mut T) -> isoprenoid::runtime::Propagation>,
	) {
		match *self {}
	}
}

pub struct OpaqueAsyncFuture<T> {
	pub(crate) _phantom: PhantomData<T>,
	pub(crate) _vacant: Opaque,
}

/// # Safety
///
/// `OpaqueFuture` is vacant.
unsafe impl<T> Send for OpaqueAsyncFuture<T> {}

impl<T> Future for OpaqueAsyncFuture<T> {
	type Output = T;

	fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
		match self._vacant {}
	}
}

pub struct OpaqueEagerFuture<T> {
	pub(crate) _phantom: PhantomData<T>,
	pub(crate) _vacant: Opaque,
}

/// # Safety
///
/// `OpaqueFuture` is vacant.
unsafe impl<T> Send for OpaqueEagerFuture<T> {}

impl<T> Future for OpaqueEagerFuture<T> {
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
