//! [`Slot`] is used in certain callbacks to ensure initialisation.
//!
//! [`Slot::write`] and [`Slot::assume_init`] yield a [`Token`] with the same invariant lifetime.
//! Iff that lifetime is sufficiently unique (i.e. because it's transient in a callback),
//! each [`Token`] proves that one matching [`Slot`] has been written to.

use core::{marker::PhantomData, mem::MaybeUninit};

/// Must be written to to create one matching [`Token`].
pub struct Slot<'a, T>(&'a mut MaybeUninit<T>, PhantomData<&'a mut &'a mut ()>);

/// Proof that one matching [`Slot`] was written to.
pub struct Token<'a>(PhantomData<&'a mut &'a mut ()>);

impl<'a, T> Slot<'a, T> {
	pub(crate) fn new(target: &'a mut MaybeUninit<T>) -> Self {
		Self(target, PhantomData)
	}

	/// Writes `value` while consuming this [`Slot`], yielding a matching [`Token`].
	pub fn write(self, value: T) -> Token<'a> {
		self.0.write(value);
		Token(PhantomData)
	}

	/// Provides exclusive access to the underlying [`MaybeUninit`].
	pub fn get_uninit(&mut self) -> &mut MaybeUninit<T> {
		&mut *self.0
	}

	/// Converts this [`Slot`] into a matching [`Token`].
	///
	/// # Safety
	///
	/// The memory this [`Slot`] points to **must** have been initialised with a valid `T`.
	pub unsafe fn assume_init(self) -> Token<'a> {
		Token(PhantomData)
	}
}
