use std::{borrow::Borrow, fmt::Debug, pin::Pin, sync::Arc};

use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef};

use crate::{raw::RawSubject, traits::Source, SourcePin};

/// Type inference helper alias for [`SubjectSR`] (using [`GlobalSignalRuntime`]).
pub type Subject<T> = SubjectSR<T, GlobalSignalRuntime>;

#[derive(Clone)]
pub struct SubjectSR<T: ?Sized + Send, SR: SignalRuntimeRef> {
    subject: Pin<Arc<RawSubject<T, SR>>>,
}

impl<T: ?Sized + Debug + Send, SR: SignalRuntimeRef + Debug> Debug for SubjectSR<T, SR>
where
    SR::Symbol: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Subject").field(&self.subject).finish()
    }
}

impl<T: Send, SR: SignalRuntimeRef> SubjectSR<T, SR> {
    pub fn new(initial_value: T) -> Self
    where
        SR: Default,
    {
        Self::with_runtime(initial_value, SR::default())
    }

    pub fn with_runtime(initial_value: T, runtime: SR) -> Self
    where
        SR: Default,
    {
        Self {
            subject: Arc::pin(RawSubject::with_runtime(initial_value, runtime)),
        }
    }

    pub fn read<'r>(&'r self) -> impl 'r + Borrow<T>
    where
        T: Sync,
    {
        self.subject.read()
    }

    pub fn read_exclusive<'r>(&'r self) -> impl 'r + Borrow<T> {
        self.subject.read_exclusive()
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

    pub fn into_get_set_blocking<'a>(self) -> (impl 'a + Clone + Fn() -> T, impl 'a + Clone + Fn(T))
    where
        Self: 'a,
        T: Sync + Send + Copy,
    {
        self.into_get_clone_set_blocking()
    }

    pub fn into_get_set<'a>(
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

    pub fn into_get_clone_set_blocking<'a>(
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

    pub fn into_get_clone_set<'a>(
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

    pub fn into_get_exclusive_set_blocking<'a>(
        self,
    ) -> (impl 'a + Clone + Fn() -> T, impl 'a + Clone + Fn(T))
    where
        Self: 'a,
        T: Copy,
    {
        self.into_get_clone_exclusive_set_blocking()
    }

    pub fn into_get_exclusive_set<'a>(
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

    pub fn into_get_clone_exclusive_set_blocking<'a>(
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

    pub fn into_get_clone_exclusive_set<'a>(
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

impl<T: Send + Sized + ?Sized, SR: ?Sized + SignalRuntimeRef> SourcePin<SR> for SubjectSR<T, SR> {
    type Output = T;

    fn touch(&self) {
        self.subject.as_ref().touch()
    }

    fn get_clone(&self) -> Self::Output
    where
        Self::Output: Sync + Clone,
    {
        self.subject.as_ref().get_clone()
    }

    fn get_clone_exclusive(&self) -> Self::Output
    where
        Self::Output: Clone,
    {
        self.subject.as_ref().get_clone_exclusive()
    }

    fn read<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Output>>
    where
        Self::Output: 'r + Sync,
    {
        self.subject.as_ref().read()
    }

    fn read_exclusive<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Output>> {
        self.subject.as_ref().read_exclusive()
    }

    fn clone_runtime_ref(&self) -> SR
    where
        SR: Sized,
    {
        self.subject.as_ref().clone_runtime_ref()
    }
}
