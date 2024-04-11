use std::{borrow::Borrow, mem, ops::Deref, pin::Pin, sync::RwLock};

use servo_arc::Arc;

use crate::raw::{RawSubject, RawSubjectGuard};

#[derive(Debug, Clone)]
pub struct Subject<T: ?Sized>(Pin<Arc<RawSubject<T>>>);

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

impl<T> Subject<T> {
    pub fn new(initial_value: T) -> Self {
        Self(unsafe {
            mem::transmute::<Arc<RawSubject<T>>, Pin<Arc<RawSubject<T>>>>(Arc::new(
                RawSubject::new(initial_value),
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

    pub fn into_get_set<'a>(
        self,
    ) -> (
        impl 'a + Clone + Send + Sync + Fn() -> T,
        impl 'a + Clone + Send + Sync + Fn(T),
    )
    where
        Self: 'a,
        T: 'static + Sync + Send + Copy,
    {
        self.into_get_clone_set()
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
    {
        let this = self.clone();
        (
            move || self.get_clone(),
            move |new_value| this.set(new_value),
        )
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
    {
        self.into_get_clone_exclusive_set()
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
    {
        let this = self.clone();
        (
            move || self.get_clone_exclusive(),
            move |new_value| this.set(new_value),
        )
    }
}
