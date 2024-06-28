use std::{
    borrow::Borrow,
    fmt::Debug,
    mem,
    pin::Pin,
    sync::{Arc, RwLock},
};

use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef};

use crate::{raw::RawSubject, AsSource, Source};

#[derive(Clone)]
pub struct Subject<T: ?Sized + Send, SR: SignalRuntimeRef = GlobalSignalRuntime>(
    Pin<Arc<RawSubject<T, SR>>>,
);

impl<T: ?Sized + std::fmt::Debug + Send, SR: SignalRuntimeRef + std::fmt::Debug> std::fmt::Debug
    for Subject<T, SR>
where
    SR::Symbol: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Subject").field(&self.0).finish()
    }
}

/// See [rust-lang#98931](https://github.com/rust-lang/rust/issues/98931).
impl<T: Send> Subject<T> {
    pub fn new(initial_value: T) -> Self
where {
        Self::with_runtime(initial_value, GlobalSignalRuntime)
    }
}

impl<T: Send, SR: SignalRuntimeRef> Subject<T, SR> {
    pub fn with_runtime(initial_value: T, runtime: SR) -> Self
    where
        SR: Default,
    {
        Self(unsafe {
            mem::transmute::<Arc<RawSubject<T, SR>>, Pin<Arc<RawSubject<T, SR>>>>(Arc::new(
                RawSubject::with_runtime(initial_value, runtime),
            ))
        })
    }

    pub fn get(&self) -> T
    where
        T: Sync + Copy,
    {
        self.0.get()
    }

    pub fn get_clone(&self) -> T
    where
        T: Sync + Clone,
    {
        self.0.get_clone()
    }

    pub fn read<'a>(&'a self) -> impl 'a + Borrow<T>
    where
        T: Sync,
    {
        self.0.read()
    }

    pub fn read_exclusive<'a>(&'a self) -> impl 'a + Borrow<T> {
        self.0.read_exclusive()
    }

    pub fn get_exclusive(&self) -> T
    where
        T: Copy,
    {
        self.0.get_exclusive()
    }

    pub fn get_clone_exclusive(&self) -> T
    where
        T: Clone,
    {
        self.0.get_clone_exclusive()
    }

    pub fn touch(&self) -> &RwLock<T> {
        self.0.touch()
    }

    pub fn set(&self, new_value: T)
    where
        T: 'static + Send,
        SR: 'static + Sync,
        SR::Symbol: Sync,
    {
        self.0.as_ref().set(new_value)
    }

    pub fn update(&self, update: impl 'static + Send + FnOnce(&mut T))
    where
        T: Send,
        SR: 'static + Sync,
        SR::Symbol: Sync,
    {
        self.0.as_ref().update(update)
    }

    pub fn set_blocking(&self, new_value: T) {
        self.0.set_blocking(new_value)
    }

    pub fn update_blocking(&self, update: impl FnOnce(&mut T)) {
        self.0.update_blocking(update)
    }

    pub fn into_get_set_blocking<'a>(self) -> (impl 'a + Clone + Fn() -> T, impl 'a + Clone + Fn(T))
    where
        Self: 'a,
        T: 'static + Sync + Send + Copy,
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
        SR: 'static + Send + Sync,
        SR::Symbol: Send + Sync,
    {
        self.into_get_clone_set()
    }

    pub fn into_get_clone_set_blocking<'a>(
        self,
    ) -> (impl 'a + Clone + Fn() -> T, impl 'a + Clone + Fn(T))
    where
        Self: 'a,
        T: 'static + Sync + Send + Clone,
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
        SR: 'static + Send + Sync,
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
        T: 'static + Send + Copy,
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
        SR: 'static + Send + Sync,
        SR::Symbol: Send + Sync,
    {
        self.into_get_clone_exclusive_set()
    }

    pub fn into_get_clone_exclusive_set_blocking<'a>(
        self,
    ) -> (impl 'a + Clone + Fn() -> T, impl 'a + Clone + Fn(T))
    where
        Self: 'a,
        T: 'static + Send + Clone,
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
        SR: 'static + Send + Sync,
        SR::Symbol: Send + Sync,
    {
        let this = self.clone();
        (
            move || self.get_clone_exclusive(),
            move |new_value| this.set(new_value),
        )
    }

    pub fn as_source(&self) -> Pin<&dyn Source<SR, Value = T>> {
        self.0.as_ref()
    }
}

impl<'a, T: 'a + Send, SR: 'a + Sync + SignalRuntimeRef> AsSource<'a, SR> for Subject<T, SR> {
    type Source = dyn 'a + Source<SR, Value = T> + Sync;

    fn as_source(&self) -> Pin<&Self::Source> {
        self.0.as_ref()
    }
}
