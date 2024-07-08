use std::{borrow::Borrow, fmt::Debug, pin::Pin, sync::Arc};

use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef, Update};

use crate::{
    raw::RawSubject,
    traits::{Source, Subscribable},
    SignalSR, SourcePin,
};

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

    /// Cheaply creates a [`SignalSR`] handle to the managed subject.
    pub fn to_signal<'a>(&self) -> SignalSR<'a, T, SR>
    where
        T: 'a,
        SR: 'a,
    {
        SignalSR {
            source: Pin::clone(&self.subject) as Pin<Arc<dyn Subscribable<SR, Output = T>>>,
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

    pub fn change(&self, new_value: T)
    where
        T: 'static + Send + PartialEq,
        SR: Sync,
        SR::Symbol: Sync,
    {
        self.subject.as_ref().change(new_value)
    }

    pub fn replace(&self, new_value: T)
    where
        T: 'static + Send,
        SR: Sync,
        SR::Symbol: Sync,
    {
        self.subject.as_ref().replace(new_value)
    }

    pub fn update(&self, update: impl 'static + Send + FnOnce(&mut T) -> Update)
    where
        SR: Sync,
        SR::Symbol: Sync,
    {
        self.subject.as_ref().update(update)
    }

    pub async fn change_async(&self, new_value: T) -> Result<T, T>
    where
        T: Send + PartialEq,
        SR: Sync,
        SR::Symbol: Sync,
    {
        self.subject.as_ref().change_async(new_value).await
    }

    pub async fn replace_async(&self, new_value: T) -> T
    where
        T: Send,
        SR: Sync,
        SR::Symbol: Sync,
    {
        self.subject.as_ref().replace_async(new_value).await
    }

    pub async fn update_async<U: Send>(
        &self,
        update: impl Send + FnOnce(&mut T) -> (U, Update),
    ) -> U
    where
        SR: Sync,
        SR::Symbol: Sync,
    {
        self.subject.as_ref().update_async(update).await
    }

    pub fn change_blocking(&self, new_value: T) -> Result<T, T>
    where
        T: PartialEq,
    {
        self.subject.change_blocking(new_value)
    }

    pub fn replace_blocking(&self, new_value: T) -> T {
        self.subject.replace_blocking(new_value)
    }

    pub fn update_blocking<U>(&self, update: impl FnOnce(&mut T) -> (U, Update)) -> U {
        self.subject.update_blocking(update)
    }

    pub fn into_source_sender<'a, S>(self, into_sender: impl FnOnce(Self) -> S) -> (SignalSR<'a, T, SR>, S)
    where
        T: 'a + Sized,
        SR: 'a,
    {
        (self.to_signal(), into_sender(self))
    }

    pub fn into_mapped_source_sender<'a, S, R>(
        self,
        map_source: impl FnOnce(SignalSR<'a, T, SR>) -> R,
        into_sender: impl FnOnce(Self) -> S,
    ) -> (R, S)
    where
        T: 'a + Sized,
        SR: 'a,
    {
        (map_source(self.to_signal()), into_sender(self))
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
