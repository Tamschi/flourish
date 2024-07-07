use std::{borrow::Borrow, pin::Pin};

use pollinate::runtime::SignalRuntimeRef;

/// **Combinators should implement this.** Interface for "raw" (stack-pinnable) signals that have an accessible value.
///
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

    fn read<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Borrow<Self::Value>>
    where
        Self::Value: Sync;

    fn read_exclusive<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Borrow<Self::Value>>;

    fn clone_runtime_ref(&self) -> SR
    where
        SR: Sized;
}

/// **Most application code should consume this.** Interface for movable signal handles that have an accessible value.
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

/// **Combinators should implement this.** Allows [`SignalSR`](`crate::SignalSR`) and [`SubscriptionSR`](`crate::SubscriptionSR`) to manage subscriptions through conversions between each other.
pub trait Subscribable<SR: ?Sized + SignalRuntimeRef>: Send + Sync + Source<SR> {
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
