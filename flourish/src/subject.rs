use std::{
    borrow::Borrow,
    fmt::Debug,
    mem,
    ops::Deref,
    pin::Pin,
    sync::{Arc, RwLock},
};

use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef};

use crate::raw::{RawSubject, RawSubjectGuard};

#[derive(Clone)]
pub struct Subject<T: ?Sized, SR: SignalRuntimeRef = GlobalSignalRuntime>(
    Pin<Arc<RawSubject<T, SR>>>,
);

impl<T: ?Sized + std::fmt::Debug, SR: SignalRuntimeRef + std::fmt::Debug> std::fmt::Debug
    for Subject<T, SR>
where
    SR::Symbol: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Subject").field(&self.0).finish()
    }
}

pub struct SubjectGuard<'a, T>(RawSubjectGuard<'a, T>);

impl<'a, T> Deref for SubjectGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.borrow()
    }
}

impl<'a, T> Borrow<T> for SubjectGuard<'a, T> {
    fn borrow(&self) -> &T {
        self.0.borrow()
    }
}

impl<T, SR: SignalRuntimeRef> Subject<T, SR> {
    pub fn new(initial_value: T) -> Self
    where
        SR: Default,
    {
        Self::with_runtime(initial_value, SR::default())
    }

    pub fn with_runtime(initial_value: T, sr: SR) -> Self
    where
        SR: Default,
    {
        Self(unsafe {
            mem::transmute::<Arc<RawSubject<T, SR>>, Pin<Arc<RawSubject<T, SR>>>>(Arc::new(
                RawSubject::with_runtime(initial_value, sr),
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

    pub fn read<'a>(&'a self) -> SubjectGuard<'a, T>
    where
        T: Sync,
    {
        SubjectGuard(self.0.read())
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
    {
        self.0.as_ref().set(new_value)
    }

    pub fn update(&self, update: impl 'static + Send + FnOnce(&mut T))
    where
        T: Send,
    {
        self.0.as_ref().update(update)
    }

    pub fn set_blocking(&self, new_value: T) {
        self.0.set_blocking(new_value)
    }

    pub fn update_blocking(&self, update: impl FnOnce(&mut T)) {
        self.0.update_blocking(update)
    }

    pub fn into_get_set_bound<'a>(self) -> (impl 'a + Clone + Fn() -> T, impl 'a + Clone + Fn(T))
    where
        Self: 'a,
        T: 'static + Sync + Send + Copy,
    {
        self.into_get_clone_set_bound()
    }

    pub fn into_get_set<'a>(
        self,
    ) -> (
        impl 'a + Clone + Send + Sync + Fn() -> T,
        impl 'a + Clone + Send + Sync + Fn(T),
    )
    where
        Self: 'a,
        T: 'static + Sync + Send + Copy,
        SR: Send + Sync,
        SR::Symbol: Send,
    {
        self.into_get_clone_set()
    }

    pub fn into_get_clone_set_bound<'a>(
        self,
    ) -> (impl 'a + Clone + Fn() -> T, impl 'a + Clone + Fn(T))
    where
        Self: 'a,
        T: 'static + Sync + Send + Clone,
    {
        let this = self.clone();
        (
            move || self.get_clone(),
            move |new_value| this.set(new_value),
        )
    }

    pub fn into_get_clone_set<'a>(
        self,
    ) -> (
        impl 'a + Clone + Send + Sync + Fn() -> T,
        impl 'a + Clone + Send + Sync + Fn(T),
    )
    where
        Self: 'a,
        T: 'static + Sync + Send + Clone,
        SR: Send + Sync,
        SR::Symbol: Send,
    {
        let this = self.clone();
        (
            move || self.get_clone(),
            move |new_value| this.set(new_value),
        )
    }

    pub fn into_get_exclusive_set_bound<'a>(
        self,
    ) -> (impl 'a + Clone + Fn() -> T, impl 'a + Clone + Fn(T))
    where
        Self: 'a,
        T: 'static + Send + Copy,
    {
        self.into_get_clone_exclusive_set_bound()
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
        SR::Symbol: Send,
    {
        self.into_get_clone_exclusive_set()
    }

    pub fn into_get_clone_exclusive_set_bound<'a>(
        self,
    ) -> (impl 'a + Clone + Fn() -> T, impl 'a + Clone + Fn(T))
    where
        Self: 'a,
        T: 'static + Send + Clone,
    {
        let this = self.clone();
        (
            move || self.get_clone_exclusive(),
            move |new_value| this.set(new_value),
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
        SR::Symbol: Send,
    {
        let this = self.clone();
        (
            move || self.get_clone_exclusive(),
            move |new_value| this.set(new_value),
        )
    }
}
