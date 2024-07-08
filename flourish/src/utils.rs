use std::mem::{size_of, MaybeUninit};

pub(crate) fn conjure_zst<T: Copy>() -> T {
	assert_eq!(size_of::<T>(), 0);
	unsafe { MaybeUninit::zeroed().assume_init() }
}
