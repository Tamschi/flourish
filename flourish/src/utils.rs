use std::mem::{size_of, MaybeUninit};

/// # Safety
///
/// Only safe if the caller could [`Copy`] a T instead.
pub(crate) unsafe fn conjure_zst<T: Copy>() -> T {
	const {
		if size_of::<T>() > 0 {
			panic!("Tried to conjure non-ZST instance.")
		}
	}
	unsafe { MaybeUninit::zeroed().assume_init() }
}
