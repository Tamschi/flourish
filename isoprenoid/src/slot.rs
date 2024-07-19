//! [`Slot`] is used in certain callbacks to ensure initialisation.
//!
//! [`Slot::write`] yield a [`Token`] with the same (invariant) lifetime.

use core::{marker::PhantomData, mem::MaybeUninit};
use std::ops::{Deref, DerefMut};

/// Must be written to before the closure returns.
pub struct Slot<'a, T>(&'a mut MaybeUninit<T>, PhantomData<&'a mut &'a mut ()>);

/// Proof that a [`Slot`] was written to.
pub struct Written<'a, T> {
	written: &'a mut T,
	_phantom: PhantomData<&'a mut &'a mut ()>,
}

impl<'a, T> Slot<'a, T> {
	pub(crate) fn new(target: &'a mut MaybeUninit<T>) -> Self {
		Self(target, PhantomData)
	}

	/// Writes `value` while consuming the [`Slot`], yielding a [`Token`].
	pub fn write(self, value: T) -> Written<'a, T> {
		Written {
			written: self.0.write(value),
			_phantom: PhantomData,
		}
	}

	/// Provides exclusive access to the underlying [`MaybeUninit`].
	pub unsafe fn get_uninit(&mut self) -> &mut MaybeUninit<T> {
		&mut *self.0
	}
}

impl<'a, T> Deref for Written<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		self.written
	}
}

impl<'a, T> DerefMut for Written<'a, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.written
	}
}
