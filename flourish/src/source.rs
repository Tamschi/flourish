use std::{borrow::Borrow, pin::Pin};

use pollinate::runtime::SignalRuntimeRef;

/// # Safety Notes
///
/// It's sound to transmute [`dyn Source<_>`](`Source`) between different associated `Value`s as long as that's sound and they're ABI-compatible.
///
/// Note that dropping the [`dyn Source<_>`](`Source`) dynamically **transmutes back**.
pub trait Source<SR: ?Sized + SignalRuntimeRef>: Send + Sync {
    type Value: ?Sized + Send;

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

    fn read_exclusive<'a>(self: Pin<&'a Self>) -> Box<dyn 'a + Borrow<Self::Value>>;

    fn clone_runtime_ref(&self) -> SR
    where
        SR: Sized;
}

pub trait SourcePin<SR: ?Sized + SignalRuntimeRef>: Send + Sync {
    type Value: ?Sized + Send;

    fn touch(&self);

    fn get(&self) -> Self::Value
    where
        Self::Value: Sync + Copy,
    {
        self.get_clone()
    }

    fn get_clone(&self) -> Self::Value
    where
        Self::Value: Sync + Clone;

    fn get_exclusive(&self) -> Self::Value
    where
        Self::Value: Copy,
    {
        self.get_clone_exclusive()
    }

    fn get_clone_exclusive(&self) -> Self::Value
    where
        Self::Value: Clone;

    fn read<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Value>>
    where
        Self::Value: 'r + Sync;

    fn read_exclusive<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Value>>;

    fn clone_runtime_ref(&self) -> SR
    where
        SR: Sized;
}
