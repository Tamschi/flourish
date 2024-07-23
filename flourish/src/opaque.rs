use std::{
	future::Future,
	marker::PhantomData,
	ops::Deref,
	pin::Pin,
	task::{Context, Poll},
};

use isoprenoid::runtime::SignalsRuntimeRef;

use crate::traits::{Source, SourceCell, Subscribable};

pub enum Opaque {}

impl<T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef> Source<T, SR> for Opaque {
	fn touch(self: Pin<&Self>) {
		unreachable!()
	}

	fn get_clone(self: Pin<&Self>) -> T
	where
		T: Sync + Clone,
	{
		unreachable!()
	}

	fn get_clone_exclusive(self: Pin<&Self>) -> T
	where
		T: Clone,
	{
		unreachable!()
	}

	fn read<'r>(self: Pin<&'r Self>) -> OpaqueDeref<T>
	where
		Self: Sized,
		T: 'r + Sync,
	{
		unreachable!()
	}

	type Read<'r> = OpaqueDeref<T>
	where
		Self: 'r + Sized,
		T: 'r + Sync;

	fn read_exclusive<'r>(self: Pin<&'r Self>) -> OpaqueDeref<T>
	where
		Self: Sized,
		T: 'r,
	{
		unreachable!()
	}

	type ReadExclusive<'r> = OpaqueDeref<T>
	where
		Self: 'r + Sized, T: 'r;

	fn read_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Deref<Target = T>>
	where
		T: 'r + Sync,
	{
		unreachable!()
	}

	fn read_exclusive_dyn<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Deref<Target = T>>
	where
		T: 'r,
	{
		unreachable!()
	}

	fn clone_runtime_ref(&self) -> SR
	where
		SR: Sized,
	{
		unreachable!()
	}
}

impl<T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef> Subscribable<T, SR> for Opaque {
	fn subscribe_inherently(self: Pin<&Self>) -> bool {
		unreachable!()
	}

	fn unsubscribe_inherently(self: Pin<&Self>) -> bool {
		unreachable!()
	}
}

impl<T: ?Sized + Send, SR: ?Sized + SignalsRuntimeRef> SourceCell<T, SR> for Opaque {
	fn change(self: Pin<&Self>, _: T)
	where
		T: 'static + Sized + PartialEq,
	{
		unreachable!()
	}

	fn replace(self: Pin<&Self>, _: T)
	where
		T: 'static + Sized,
	{
		unreachable!()
	}

	fn update(
		self: Pin<&Self>,
		_: impl 'static + Send + FnOnce(&mut T) -> isoprenoid::runtime::Propagation,
	) where
		Self: Sized,
		T: 'static,
	{
		unreachable!()
	}

	fn update_dyn(
		self: Pin<&Self>,
		_: Box<dyn 'static + Send + FnOnce(&mut T) -> isoprenoid::runtime::Propagation>,
	) where
		T: 'static,
	{
		unreachable!()
	}

	fn change_eager<'f>(self: Pin<&Self>, _: T) -> OpaqueFuture<Result<Result<T, T>, T>>
	where
		Self: 'f + Sized,
		T: 'f + Sized + PartialEq,
	{
		unreachable!()
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
		unreachable!()
	}

	type ReplaceEager<'f> = OpaqueFuture<Result<T, T>>
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
	) -> OpaqueFuture<Result<U, F>>
	where
		Self: 'f + Sized,
	{
		unreachable!()
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
		unreachable!()
	}

	fn replace_eager_dyn<'f>(
		self: Pin<&Self>,
		_: T,
	) -> Box<dyn 'f + Send + Future<Output = Result<T, T>>>
	where
		T: 'f + Sized,
	{
		unreachable!()
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
		unreachable!()
	}

	fn change_blocking(&self, _: T) -> Result<T, T>
	where
		T: Sized + PartialEq,
	{
		unreachable!()
	}

	fn replace_blocking(&self, _: T) -> T
	where
		T: Sized,
	{
		unreachable!()
	}

	fn update_blocking<U>(
		&self,
		_: impl FnOnce(&mut T) -> (isoprenoid::runtime::Propagation, U),
	) -> U
	where
		Self: Sized,
	{
		unreachable!()
	}

	fn update_blocking_dyn(
		&self,
		_: Box<dyn '_ + FnOnce(&mut T) -> isoprenoid::runtime::Propagation>,
	) {
		unreachable!()
	}
}

pub(crate) struct OpaqueFuture<T> {
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
		unreachable!()
	}
}

pub(crate) struct OpaqueDeref<T: ?Sized> {
	pub(crate) _phantom: PhantomData<T>,
	pub(crate) _vacant: Opaque,
}

impl<T: ?Sized> Deref for OpaqueDeref<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		unreachable!()
	}
}
