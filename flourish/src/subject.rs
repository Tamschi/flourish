use std::{
    borrow::Borrow,
    ops::Deref,
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

    fn set(&self, new_value: T)
    where
        T: Send,
    {
        self.update(|value| *value = new_value);
    }

    fn update(&self, update: impl 'static + Send + FnOnce(&mut T))
    where
        T: Send,
    {
        self.source
            .update(|_, _| update(&mut *self.value.write().unwrap()))
    }

    fn set_blocking(&self, new_value: T) {
        self.update_blocking(|value| *value = new_value);
    }

    fn update_blocking(&self, update: impl 'static + Send + FnOnce(&mut T)) {
        self.source
            .update_blocking(|_, _| update(&mut *self.value.write().unwrap()))
    }
}
