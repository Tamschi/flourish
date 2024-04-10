use std::{borrow::Borrow, ops::Deref};

use servo_arc::Arc;

use crate::raw::{RawSubject, RawSubjectGuard};

#[derive(Debug, Clone)]
pub struct Subject<T: ?Sized>(Arc<RawSubject<T>>);

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
        Self(Arc::new(RawSubject::new(initial_value)))
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

    pub fn touch(&self) {
        self.0.touch()
    }

    // fn set(&self, new_value: T)
    // where
    //     T: 'static + Send,
    // {
    //     self.update(|value| *value = new_value);
    // }

    // fn update(&self, update: impl 'static + Send + FnOnce(&mut T))
    // where
    //     T: Send,
    // {
    //     Pin::new(&self.source).update(|_, _| update(&mut *self.value.write().unwrap()))
    // }

    pub fn set_blocking(&self, new_value: T) {
        self.0.set_blocking(new_value)
    }

    pub fn update_blocking(&self, update: impl FnOnce(&mut T)) {
        self.0.update_blocking(update)
    }

    // fn into_get_set<'a>(self) -> (impl 'a + Fn() -> T, impl 'a + Fn(T))
    // where
    //     Self: 'a,
    //     T: 'static + Sync + Send + Copy,
    // {
    //     self.into_get_clone_set()
    // }

    // fn into_get_clone_set<'a>(self) -> (impl 'a + Fn() -> T, impl 'a + Fn(T))
    // where
    //     Self: 'a,
    //     T: 'static + Sync + Send + Clone,
    // {
    //     let this1 = Arc::pin(self);
    //     let this2 = Pin::clone(&this1);
    //     (
    //         move || this1.get_clone(),
    //         move |new_value| this2.set(new_value),
    //     )
    // }

    // fn into_get_exclusive_set<'a>(self: Pin<Arc<Self>>) -> (impl 'a + Fn() -> T, impl 'a + Fn(T))
    // where
    //     Self: 'a,
    //     T: 'static + Send + Copy,
    // {
    //     self.into_get_clone_exclusive_set()
    // }

    // fn into_get_clone_exclusive_set<'a>(
    //     self: Pin<Arc<Self>>,
    // ) -> (impl 'a + Fn() -> T, impl 'a + Fn(T))
    // where
    //     Self: 'a,
    //     T: 'static + Send + Clone,
    // {
    //     let this1 = Arc::pin(self);
    //     let this2 = Pin::clone(&this1);
    //     (
    //         move || this1.get_clone_exclusive(),
    //         move |new_value| this2.set(new_value),
    //     )
    // }
}
