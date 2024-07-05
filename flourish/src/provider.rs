use std::{borrow::Borrow, fmt::Debug, pin::Pin, sync::Arc};

use pollinate::runtime::{CallbackTableTypes, GlobalSignalRuntime, SignalRuntimeRef};

use crate::{raw::RawProvider, Source, SourcePin};

pub type Provider<'a, T> = ProviderSR<'a, T, GlobalSignalRuntime>;

pub struct ProviderSR<'a, T: 'a + ?Sized + Send, SR: 'a + SignalRuntimeRef> {
    subject: Pin<
        Arc<
            RawProvider<
                T,
                Box<
                    dyn 'a
                        + Send
                        + FnMut(<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
                >,
                SR,
            >,
        >,
    >,
}

impl<'a, T: 'a + ?Sized + Send, SR: 'a + SignalRuntimeRef> Clone for ProviderSR<'a, T, SR> {
    fn clone(&self) -> Self {
        Self {
            subject: self.subject.clone(),
        }
    }
}

impl<'a, T: ?Sized + Debug + Send, SR: SignalRuntimeRef + Debug> Debug for ProviderSR<'a, T, SR>
where
    SR::Symbol: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        //FIXME: This could be more informative.
        f.debug_struct("Provider").finish_non_exhaustive()
    }
}

/// See [rust-lang#98931](https://github.com/rust-lang/rust/issues/98931).
impl<'a, T: Send, SR: SignalRuntimeRef> ProviderSR<'a, T, SR> {
    pub fn new(
        initial_value: T,
        handler: impl 'a
            + Send
            + FnMut(<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
    ) -> Self
    where
        SR: Default,
    {
        Self::with_runtime(initial_value, handler, SR::default())
    }

    pub fn with_runtime(
        initial_value: T,
        handler: impl 'a
            + Send
            + FnMut(<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
        runtime: SR,
    ) -> Self
    where
        SR: Default,
    {
        Self {
            subject: Arc::pin(RawProvider::with_runtime(
                initial_value,
                Box::new(handler),
                runtime,
            )),
        }
    }

    pub fn set(&self, new_value: T)
    where
        T: 'static + Send,
        SR: Sync,
        SR::Symbol: Sync,
    {
        self.subject.as_ref().set(new_value)
    }

    pub fn update(&self, update: impl 'static + Send + FnOnce(&mut T))
    where
        SR: Sync,
        SR::Symbol: Sync,
    {
        self.subject.as_ref().update(update)
    }

    pub fn set_blocking(&self, new_value: T) {
        self.subject.set_blocking(new_value)
    }

    pub fn update_blocking(&self, update: impl FnOnce(&mut T)) {
        self.subject.update_blocking(update)
    }

    pub fn into_get_set_blocking(self) -> (impl 'a + Clone + Fn() -> T, impl 'a + Clone + Fn(T))
    where
        Self: 'a,
        T: Sync + Send + Copy,
    {
        self.into_get_clone_set_blocking()
    }

    pub fn into_get_set(
        self,
    ) -> (
        impl 'a + Clone + Send + Sync + Unpin + Fn() -> T,
        impl 'a + Clone + Send + Sync + Unpin + Fn(T),
    )
    where
        Self: 'a,
        T: 'static + Sync + Send + Copy,
        SR: Send + Sync,
        SR::Symbol: Send + Sync,
    {
        self.into_get_clone_set()
    }

    pub fn into_get_clone_set_blocking(
        self,
    ) -> (impl 'a + Clone + Fn() -> T, impl 'a + Clone + Fn(T))
    where
        Self: 'a,
        T: Sync + Send + Clone,
    {
        let this = self.clone();
        (
            move || self.get_clone(),
            move |new_value| this.set_blocking(new_value),
        )
    }

    pub fn into_get_clone_set(
        self,
    ) -> (
        impl 'a + Clone + Send + Sync + Unpin + Fn() -> T,
        impl 'a + Clone + Send + Sync + Unpin + Fn(T),
    )
    where
        Self: 'a,
        T: 'static + Sync + Send + Clone,
        SR: Send + Sync,
        SR::Symbol: Send + Sync,
    {
        let this = self.clone();
        (
            move || self.get_clone(),
            move |new_value| this.set(new_value),
        )
    }

    pub fn into_get_exclusive_set_blocking(
        self,
    ) -> (impl 'a + Clone + Fn() -> T, impl 'a + Clone + Fn(T))
    where
        Self: 'a,
        T: Copy,
    {
        self.into_get_clone_exclusive_set_blocking()
    }

    pub fn into_get_exclusive_set(
        self,
    ) -> (
        impl 'a + Clone + Send + Sync + Fn() -> T,
        impl 'a + Clone + Send + Sync + Fn(T),
    )
    where
        Self: 'a,
        T: 'static + Send + Copy,
        SR: Send + Sync,
        SR::Symbol: Send + Sync,
    {
        self.into_get_clone_exclusive_set()
    }

    pub fn into_get_clone_exclusive_set_blocking(
        self,
    ) -> (impl 'a + Clone + Fn() -> T, impl 'a + Clone + Fn(T))
    where
        Self: 'a,
        T: Clone,
    {
        let this = self.clone();
        (
            move || self.get_clone_exclusive(),
            move |new_value| this.set_blocking(new_value),
        )
    }

    pub fn into_get_clone_exclusive_set(
        self,
    ) -> (
        impl 'a + Clone + Send + Sync + Fn() -> T,
        impl 'a + Clone + Send + Sync + Fn(T),
    )
    where
        Self: 'a,
        T: 'static + Send + Clone,
        SR: Send + Sync,
        SR::Symbol: Send + Sync,
    {
        let this = self.clone();
        (
            move || self.get_clone_exclusive(),
            move |new_value| this.set(new_value),
        )
    }
}

impl<'a, T: Send + Sized + ?Sized, SR: ?Sized + SignalRuntimeRef> SourcePin<SR>
    for ProviderSR<'a, T, SR>
{
    type Value = T;

    fn touch(&self) {
        self.subject.as_ref().touch();
    }

    fn get_clone(&self) -> Self::Value
    where
        Self::Value: Sync + Clone,
    {
        self.subject.as_ref().get_clone()
    }

    fn get_clone_exclusive(&self) -> Self::Value
    where
        Self::Value: Clone,
    {
        self.subject.as_ref().get_clone_exclusive()
    }

    fn read<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Value>>
    where
        Self::Value: 'r + Sync,
    {
        self.subject.as_ref().read()
    }

    fn read_exclusive<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Value>> {
        self.subject.as_ref().read_exclusive()
    }

    fn clone_runtime_ref(&self) -> SR
    where
        SR: Sized,
    {
        self.subject.as_ref().clone_runtime_ref()
    }
}
