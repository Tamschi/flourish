use std::{borrow::Borrow, pin::Pin};

use pollinate::runtime::SignalRuntimeRef;

pub trait Source<SR: ?Sized + SignalRuntimeRef> {
    type Value: ?Sized;

    fn touch(self: Pin<&Self>);

    fn get(self: Pin<&Self>) -> Self::Value
    where
        Self::Value: Sync + Copy,
    {
        self.get_clone()
    }

    fn get_clone(self: Pin<&Self>) -> Self::Value
    where
        Self::Value: Sync + Clone;

    fn get_exclusive(self: Pin<&Self>) -> Self::Value
    where
        Self::Value: Copy,
    {
        self.get_clone_exclusive()
    }

    fn get_clone_exclusive(self: Pin<&Self>) -> Self::Value
    where
        Self::Value: Clone;

    fn read<'a>(self: Pin<&'a Self>) -> Box<dyn 'a + Borrow<Self::Value>>
    where
        Self::Value: 'a + Sync;
}

impl<F: ?Sized + Fn() -> T, T, SR: ?Sized + SignalRuntimeRef> Source<SR> for F {
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
        Self::Value: Clone,
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

pub trait AsSource<'a, SR: SignalRuntimeRef> {
    type Source: 'a + ?Sized;
    fn as_source(&self) -> Pin<&Self::Source>;
}

impl<'a, T: 'a + ?Sized, SR: SignalRuntimeRef> AsSource<'a, SR> for Pin<&T>
where
    T: Source<SR>,
{
    type Source = T;

    fn as_source(&self) -> Pin<&Self::Source> {
        self.as_ref()
    }
}
