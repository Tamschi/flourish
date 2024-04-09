#![warn(clippy::pedantic)]

use std::{cell::UnsafeCell, mem, num::NonZeroU64, pin::Pin, ptr::NonNull};

pub struct SelfHandle(NonZeroU64);

impl Drop for SelfHandle {
    fn drop(&mut self) {
        todo!()
    }
}

pub unsafe fn init<T: Send>(
    receiver: Pin<NonNull<T>>,
    init: unsafe extern "C" fn(receiver: Pin<&mut T>),
    eval: unsafe extern "C" fn(receiver: Pin<&T>),
) -> SelfHandle {
    todo!()
}

pub fn tag(self_handle: &SelfHandle) {
    todo!()
}

pub trait GetPinNonNullExt {
    type Target: ?Sized;
    fn get_pin_non_null(self: Pin<&Self>) -> Pin<NonNull<Self::Target>>;
}

impl<T: ?Sized> GetPinNonNullExt for UnsafeCell<T> {
    type Target = T;

    fn get_pin_non_null(self: Pin<&Self>) -> Pin<NonNull<Self::Target>> {
        let pointer = self.get();
        unsafe {
            // SAFETY: Memory layout guaranteed by `#[repr(transparent)]` on `Pin<…>` and `NonNull<…>`.
            mem::transmute::<*mut T, Pin<NonNull<T>>>(pointer)
        }
    }
}
