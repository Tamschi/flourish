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
        Self::Value: Sync;

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

pub trait Subscribable<SR: ?Sized + SignalRuntimeRef>: Send + Sync {
    type Value: ?Sized + Send;

    fn pull<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Borrow<Self::Value>>;

    /// Unsubscribes this [`Subscribable`] (only regarding innate subscription!).
    ///
    /// # Returns
    ///
    /// Whether this instance was previously innately subscribed.
    ///
    /// An innate subscription is a subscription not caused by a dependent subscriber.
    fn unsubscribe(self: Pin<&Self>) -> bool;
}

/// # Safety
///
/// Both `ref_as_source` and `ref_as_subscribable` must be casts with identical data pointer!
pub unsafe trait SubscribableSource<SR: ?Sized + SignalRuntimeRef>: Sync + Send {
    type Value: ?Sized + Send;

    fn ref_as_source(self: Pin<&Self>) -> Pin<&dyn Source<SR, Value = Self::Value>>;

    fn ref_as_subscribable(self: Pin<&Self>) -> Pin<&dyn Subscribable<SR, Value = Self::Value>>;

    fn clone_runtime_ref(&self) -> SR;
}

unsafe impl<S: Sized + Send + Sync, T: ?Sized + Send, SR: ?Sized + SignalRuntimeRef>
    SubscribableSource<SR> for S
where
    S: Sized + Source<SR, Value = T> + Subscribable<SR, Value = T>,
{
    type Value = T;

    fn ref_as_source(self: Pin<&Self>) -> Pin<&dyn Source<SR, Value = T>> {
        self
    }

    fn ref_as_subscribable(self: Pin<&Self>) -> Pin<&dyn Subscribable<SR, Value = T>> {
        self
    }

    fn clone_runtime_ref(&self) -> SR {
        self.clone_runtime_ref()
    }
}
