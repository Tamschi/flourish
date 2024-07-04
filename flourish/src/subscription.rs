use std::{borrow::Borrow, pin::Pin, sync::Arc};

use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef, Update};

use crate::{
    raw::{computed, folded, merged},
    traits::Subscribable,
    SourcePin,
};

pub type Subscription<'a, T> = SubscriptionSR<'a, T, GlobalSignalRuntime>;

#[must_use = "Subscriptions are cancelled when dropped."]
pub struct SubscriptionSR<
    'a,
    T: 'a + Send + ?Sized + Clone,
    SR: 'a + ?Sized + SignalRuntimeRef = GlobalSignalRuntime,
> {
    source: Pin<Arc<dyn 'a + Subscribable<SR, Value = T>>>,
}

unsafe impl<'a, T: 'a + Send + ?Sized + Clone, SR: 'a + ?Sized + SignalRuntimeRef> Send
    for SubscriptionSR<'a, T, SR>
{
}
unsafe impl<'a, T: 'a + Send + ?Sized + Clone, SR: 'a + ?Sized + SignalRuntimeRef> Sync
    for SubscriptionSR<'a, T, SR>
{
}

impl<'a, T: 'a + Send + ?Sized + Clone, SR: 'a + ?Sized + SignalRuntimeRef> Drop
    for SubscriptionSR<'a, T, SR>
{
    fn drop(&mut self) {
        self.source.as_ref().unsubscribe();
    }
}

/// See [rust-lang#98931](https://github.com/rust-lang/rust/issues/98931).
impl<'a, T: 'a + Send + ?Sized + Clone, SR: SignalRuntimeRef> SubscriptionSR<'a, T, SR> {
    pub fn new<S: 'a + Subscribable<SR, Value = T>>(source: S) -> Self {
        source.clone_runtime_ref().run_detached(|| {
            let arc = Arc::pin(source);
            arc.as_ref().pull();
            Self { source: arc }
        })
    }

    pub fn computed(f: impl 'a + Send + FnMut() -> T) -> Self
    where
        SR: Default,
    {
        Self::new(computed(f, SR::default()))
    }

    pub fn computed_with_runtime(f: impl 'a + Send + FnMut() -> T, runtime: SR) -> Self {
        Self::new(computed(f, runtime))
    }

    /// This is a convenience method. See [`folded`](`folded()`).
    pub fn folded(init: T, f: impl 'a + Send + FnMut(&mut T) -> Update) -> Self
    where
        SR: Default,
    {
        Self::new(folded(init, f, SR::default()))
    }

    /// This is a convenience method. See [`folded`](`folded()`).
    pub fn folded_with_runtime(
        init: T,
        f: impl 'a + Send + FnMut(&mut T) -> Update,
        runtime: SR,
    ) -> Self {
        Self::new(folded(init, f, runtime))
    }

    /// This is a convenience method. See [`merged`](`merged()`).
    pub fn merged(
        select: impl 'a + Send + FnMut() -> T,
        merge: impl 'a + Send + FnMut(&mut T, T) -> Update,
    ) -> Self
    where
        SR: Default,
    {
        Self::new(merged(select, merge, SR::default()))
    }

    /// This is a convenience method. See [`merged`](`merged()`).
    pub fn merged_with_runtime(
        select: impl 'a + Send + FnMut() -> T,
        merge: impl 'a + Send + FnMut(&mut T, T) -> Update,
        runtime: SR,
    ) -> Self {
        Self::new(merged(select, merge, runtime))
    }
}

impl<'a, T: 'a + Send + ?Sized + Clone, SR: 'a + ?Sized + SignalRuntimeRef> SourcePin<SR>
    for SubscriptionSR<'a, T, SR>
{
    type Value = T;

    fn touch(&self) {
        self.source.as_ref().touch()
    }

    fn get_clone(&self) -> Self::Value
    where
        Self::Value: Sync + Clone,
    {
        self.source.as_ref().get_clone()
    }

    fn get_clone_exclusive(&self) -> Self::Value
    where
        Self::Value: Clone,
    {
        self.source.as_ref().get_clone_exclusive()
    }

    fn read<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Value>>
    where
        Self::Value: 'r + Sync,
    {
        self.source.as_ref().read()
    }

    fn read_exclusive<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Value>> {
        self.source.as_ref().read_exclusive()
    }

    fn clone_runtime_ref(&self) -> SR
    where
        SR: Sized,
    {
        self.source.as_ref().clone_runtime_ref()
    }
}

// TODO: `unsubscribe(self)` to convert into `SignalSR`, `to_signal(&self)`.
