use std::{
    borrow::Borrow,
    ops::Deref,
    pin::Pin,
    sync::{Arc, RwLock, RwLockReadGuard},
};

use pin_project::pin_project;
use pollinate::Source;

#[pin_project]
pub struct Subject<T> {
    #[pin]
    source: Source<(), ()>, // Unpin
    value: RwLock<T>,
}

/// TODO: Safety.
unsafe impl<T> Sync for Subject<T> {}

pub struct SubjectGuard<'a, T>(RwLockReadGuard<'a, T>);

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
    pub fn new(initial_value: T) -> Arc<Self> {
        Arc::new(Self::new_raw(initial_value))
    }

    pub fn new_raw(initial_value: T) -> Self {
        Self {
            source: Source::new(()),
            value: initial_value.into(),
        }
    }

    pub fn get(&self) -> T
    where
        T: Sync + Copy,
    {
        *self.read()
    }

    pub fn get_clone(&self) -> T
    where
        T: Sync + Clone,
    {
        self.read().clone()
    }

    pub fn read<'a>(&'a self) -> SubjectGuard<'a, T>
    where
        T: Sync,
    {
        self.touch();
        SubjectGuard(self.value.read().unwrap())
    }

    pub fn get_mut<'a>(&'a mut self) -> &mut T {
        self.value.get_mut().unwrap()
    }

    pub fn get_exclusive(&self) -> T
    where
        T: Copy,
    {
        self.get_clone_exclusive()
    }

    pub fn get_clone_exclusive(&self) -> T
    where
        T: Clone,
    {
        self.touch();
        self.value.write().unwrap().clone()
    }

    fn touch(&self) {
        self.source.tag();
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
        self.update_blocking(|value| *value = new_value);
    }

    pub fn update_blocking(&self, update: impl FnOnce(&mut T)) {
        Pin::new(&self.source).update_blocking(|_, _| update(&mut *self.value.write().unwrap()))
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
