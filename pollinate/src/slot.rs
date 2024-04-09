use std::{marker::PhantomData, mem::MaybeUninit};

pub struct Slot<'a, T>(&'a mut MaybeUninit<T>, PhantomData<&'a mut &'a mut ()>);
pub struct Token<'a>(PhantomData<&'a mut &'a mut ()>);

impl<'a, T> Slot<'a, T> {
    pub fn write(self, value: T) -> Token<'a> {
        self.0.write(value);
        Token(PhantomData)
    }

    pub unsafe fn get_uninit(&mut self) -> &mut MaybeUninit<T> {
        &mut *self.0
    }

    pub unsafe fn assume_init(self) -> Token<'a> {
        Token(PhantomData)
    }
}
