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
    /// The type of value presented by the [`Source`].
    type Output: ?Sized + Send;

    /// Records `self` as dependency without accessing the value.
    fn touch(self: Pin<&Self>);

    /// Records `self` as dependency and retrieves a copy of the value.
    ///
    /// Prefer [`Source::touch`] where possible.
    fn get(self: Pin<&Self>) -> Self::Output
    where
        Self::Output: Sync + Copy,
    {
        self.get_clone()
    }

    /// Records `self` as dependency and retrieves a clone of the value.
    ///
    /// Prefer [`Source::get`] where available.
    fn get_clone(self: Pin<&Self>) -> Self::Output
    where
        Self::Output: Sync + Clone;

    /// Records `self` as dependency and retrieves a copy of the value.
    ///
    /// Prefer [`Source::get`] where available.
    fn get_exclusive(self: Pin<&Self>) -> Self::Output
    where
        Self::Output: Copy,
    {
        self.get_clone_exclusive()
    }

    /// Records `self` as dependency and retrieves a clone of the value.
    ///
    /// Prefer [`Source::get_clone`] where available.
    fn get_clone_exclusive(self: Pin<&Self>) -> Self::Output
    where
        Self::Output: Clone;

    /// Records `self` as dependency and allows borrowing the value.
    ///
    /// Prefer a type-associated `.read()` method where available.
    fn read<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Borrow<Self::Output>>
    where
        Self::Output: Sync;

    /// Records `self` as dependency and allows borrowing the value.
    ///
    /// Prefer a type-associated `.read()` method where available.  
    /// Otherwise, prefer a type-associated `.read_exclusive()` method where available.  
    /// Otherwise, prefer [`Source::read`] where available.
    fn read_exclusive<'r>(self: Pin<&'r Self>) -> Box<dyn 'r + Borrow<Self::Output>>;

    /// Clones this [`SourcePin`]'s [`SignalRuntimeRef`].
    fn clone_runtime_ref(&self) -> SR
    where
        SR: Sized;
}

/// **Most application code should consume this.** Interface for movable signal handles that have an accessible value.
pub trait SourcePin<SR: ?Sized + SignalRuntimeRef>: Send + Sync {
    /// The type of value presented by the [`SourcePin`].
    type Output: ?Sized + Send;

    /// Records `self` as dependency without accessing the value.
    fn touch(&self);

    /// Records `self` as dependency and retrieves a copy of the value.
    ///
    /// Prefer [`SourcePin::touch`] where possible.
    fn get(&self) -> Self::Output
    where
        Self::Output: Sync + Copy,
    {
        self.get_clone()
    }

    /// Records `self` as dependency and retrieves a clone of the value.
    ///
    /// Prefer [`SourcePin::get`] where available.
    fn get_clone(&self) -> Self::Output
    where
        Self::Output: Sync + Clone;

    /// Records `self` as dependency and retrieves a copy of the value.
    ///
    /// Prefer [`SourcePin::get`] where available.
    fn get_exclusive(&self) -> Self::Output
    where
        Self::Output: Copy,
    {
        self.get_clone_exclusive()
    }

    /// Records `self` as dependency and retrieves a clone of the value.
    ///
    /// Prefer [`SourcePin::get_clone`] where available.
    fn get_clone_exclusive(&self) -> Self::Output
    where
        Self::Output: Clone;

    /// Records `self` as dependency and allows borrowing the value.
    ///
    /// Prefer a type-associated `.read()` method where available.
    fn read<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Output>>
    where
        Self::Output: 'r + Sync;

    /// Records `self` as dependency and allows borrowing the value.
    ///
    /// Prefer a type-associated `.read()` method where available.  
    /// Otherwise, prefer a type-associated `.read_exclusive()` method where available.  
    /// Otherwise, prefer [`SourcePin::read`] where available.
    fn read_exclusive<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Output>>;

    /// Clones this [`SourcePin`]'s [`SignalRuntimeRef`].
    fn clone_runtime_ref(&self) -> SR
    where
        SR: Sized;
}

/// **Combinators should implement this.** Allows [`SignalSR`](`crate::SignalSR`) and [`SubscriptionSR`](`crate::SubscriptionSR`) to manage subscriptions through conversions between each other.
pub trait Subscribable<SR: ?Sized + SignalRuntimeRef>: Send + Sync + Source<SR> {
    /// Subscribes this [`Subscribable`] (only regarding innate subscription)!
    ///
    /// If necessary, this instance is initialised first, so that callbacks are active for it.
    ///
    /// # Logic
    ///
    /// The implementor **must** ensure dependencies are evaluated and current iff [`Some`] is returned.
    ///
    /// Iff this method is called in parallel, initialising and subscribing calls **may** differ!
    ///
    /// # Returns
    ///
    /// [`Some`] iff the inherent subscription is new, otherwise [`None`].
    fn subscribe_inherently<'r>(self: Pin<&'r Self>) -> Option<Box<dyn 'r + Borrow<Self::Output>>>;

    /// Unsubscribes this [`Subscribable`] (only regarding innate subscription!).
    ///
    /// # Returns
    ///
    /// Whether this instance was previously innately subscribed.
    ///
    /// An innate subscription is a subscription not caused by a dependent subscriber.
    fn unsubscribe_inherently(self: Pin<&Self>) -> bool;
}
