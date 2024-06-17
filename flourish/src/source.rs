use std::{borrow::Borrow, pin::Pin};

pub trait Source {
    type Value: ?Sized;

    fn touch(self: Pin<&Self>);

    fn get(self: Pin<&Self>) -> Self::Value
    where
        Self::Value: Sync + Copy;

    fn get_clone(self: Pin<&Self>) -> Self::Value
    where
        Self::Value: Sync + Clone;

    fn get_exclusive(self: Pin<&Self>) -> Self::Value
    where
        Self::Value: Copy;

    fn get_clone_exclusive(self: Pin<&Self>) -> Self::Value
    where
        Self::Value: Copy;

    fn read<'a>(self: Pin<&'a Self>) -> Box<dyn 'a + Borrow<Self::Value>>
    where
        Self::Value: 'a + Sync;
}

impl<F: ?Sized + Fn() -> T, T> Source for F {
    type Value = T;

    fn touch(self: Pin<&Self>) {
        self();
    }

    fn get(self: Pin<&Self>) -> Self::Value
    where
        Self::Value: Sync + Copy,
    {
        self()
    }

    fn get_clone(self: Pin<&Self>) -> Self::Value
    where
        Self::Value: Sync + Clone,
    {
        self()
    }

    fn get_exclusive(self: Pin<&Self>) -> Self::Value
    where
        Self::Value: Copy,
    {
        self()
    }

    fn get_clone_exclusive(self: Pin<&Self>) -> Self::Value
    where
        Self::Value: Copy,
    {
        self()
    }

    fn read<'a>(self: Pin<&'a Self>) -> Box<dyn 'a + Borrow<Self::Value>>
    where
        Self::Value: 'a + Sync,
    {
        Box::new(self())
    }
}
