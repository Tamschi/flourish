//! [`Slot`] is used in certain callbacks to ensure initialisation.
//!
//! [`Slot::write`] yield a [`Token`] with the same (invariant) lifetime.

use core::{marker::PhantomData, mem::MaybeUninit};

/// Must be written to before the closure returns.
pub struct Slot<'a, T>(&'a mut MaybeUninit<T>, PhantomData<&'a mut &'a mut ()>);

/// Proof that a [`Slot`] was written to.
pub struct Token<'a>(PhantomData<&'a mut &'a mut ()>);

impl<'a, T> Slot<'a, T> {
    pub(crate) fn new(target: &'a mut MaybeUninit<T>) -> Self {
        Self(target, PhantomData)
    }

    /// Writes `value` while consuming the [`Slot`], yielding a [`Token`].
    pub fn write(self, value: T) -> Token<'a> {
        self.0.write(value);
        Token(PhantomData)
    }

    /// Provides exclusive access to the underlying [`MaybeUninit`].
    pub unsafe fn get_uninit(&mut self) -> &mut MaybeUninit<T> {
        &mut *self.0
    }

    /// Creates a [`Token`] from this [`Slot`].
    pub unsafe fn assume_init(self) -> Token<'a> {
        Token(PhantomData)
    }
}
