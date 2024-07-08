use std::{
    borrow::Borrow,
    fmt::{self, Debug, Formatter},
    mem::{needs_drop, size_of},
    pin::Pin,
    sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use pin_project::pin_project;
use pollinate::{
    raw::{NoCallbacks, RawSignal},
    runtime::SignalRuntimeRef,
};

use crate::utils::conjure_zst;

use super::{Source, Subscribable};

#[pin_project]
pub struct RawSubject<T: ?Sized + Send, SR: SignalRuntimeRef> {
    #[pin]
    signal: RawSignal<AssertSync<RwLock<T>>, (), SR>,
}

impl<T: ?Sized + Send + Debug, SR: SignalRuntimeRef + Debug> Debug for RawSubject<T, SR>
where
    SR::Symbol: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawSubject")
            .field("signal", &&self.signal)
            .finish()
    }
}

/// TODO: Safety.
unsafe impl<T: Send, SR: SignalRuntimeRef + Sync> Sync for RawSubject<T, SR> {}

struct AssertSync<T: ?Sized>(T);
unsafe impl<T: ?Sized> Sync for AssertSync<T> {}

impl<T: Debug + ?Sized> Debug for AssertSync<RwLock<T>> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let maybe_guard = self.0.try_write();
        f.debug_tuple("AssertSync")
            .field(
                maybe_guard
                    .as_ref()
                    .map_or_else(|_| &"(locked)" as &dyn Debug, |guard| guard),
            )
            .finish()
    }
}

struct RawSubjectGuard<'a, T: ?Sized>(RwLockReadGuard<'a, T>);
struct RawSubjectGuardExclusive<'a, T: ?Sized>(RwLockWriteGuard<'a, T>);

impl<'a, T: ?Sized> Borrow<T> for RawSubjectGuard<'a, T> {
    fn borrow(&self) -> &T {
        self.0.borrow()
    }
}

impl<'a, T: ?Sized> Borrow<T> for RawSubjectGuardExclusive<'a, T> {
    fn borrow(&self) -> &T {
        self.0.borrow()
    }
}

impl<T: ?Sized + Send, SR: SignalRuntimeRef> RawSubject<T, SR> {
    pub fn new(initial_value: T) -> Self
    where
        T: Sized,
        SR: Default,
    {
        Self::with_runtime(initial_value, SR::default())
    }

    pub fn with_runtime(initial_value: T, runtime: SR) -> Self
    where
        T: Sized,
    {
        Self {
            signal: RawSignal::with_runtime(AssertSync(RwLock::new(initial_value)), runtime),
        }
    }

    pub fn get(&self) -> T
    where
        T: Sync + Copy,
    {
        if size_of::<T>() == 0 {
            // The read is unobservable, so just skip locking.
            self.touch();
            conjure_zst()
        } else {
            *self.read().borrow()
        }
    }

    pub fn get_clone(&self) -> T
    where
        T: Sync + Clone,
    {
        self.read().borrow().clone()
    }

    pub fn read<'a>(&'a self) -> impl 'a + Borrow<T>
    where
        T: Sync,
    {
        let this = &self;
        RawSubjectGuard(this.touch().read().unwrap())
    }

    pub fn read_exclusive<'a>(&'a self) -> impl 'a + Borrow<T> {
        let this = &self;
        RawSubjectGuardExclusive(this.touch().write().unwrap())
    }

    pub fn get_mut<'a>(&'a mut self) -> &mut T {
        self.signal.eager_mut().0.get_mut().unwrap()
    }

    pub fn get_exclusive(&self) -> T
    where
        T: Copy,
    {
        if size_of::<T>() == 0 {
            // The read is unobservable, so just skip locking.
            self.touch();
            conjure_zst()
        } else {
            self.get_clone_exclusive()
        }
    }

    pub fn get_clone_exclusive(&self) -> T
    where
        T: Clone,
    {
        self.touch().write().unwrap().clone()
    }

    pub(crate) fn touch(&self) -> &RwLock<T> {
        unsafe {
            // SAFETY: Doesn't defer memory access.
            &*(&Pin::new_unchecked(&self.signal)
                .project_or_init::<NoCallbacks>(|_, slot| slot.write(()))
                .0
                 .0 as *const _)
        }
    }

    pub fn set(self: Pin<&Self>, new_value: T)
    where
        T: 'static + Send + Sized,
        SR: Sync,
        SR::Symbol: Sync,
    {
        if needs_drop::<T>() || size_of::<T>() > 0 {
            self.update(|value| *value = new_value);
        } else {
            // The write is unobservable, so just skip locking.
            self.signal
                .clone_runtime_ref()
                .run_detached(|| self.touch());
            self.project_ref().signal.update(|_, _| ());
        }
    }

    pub fn update(self: Pin<&Self>, update: impl 'static + Send + FnOnce(&mut T))
    where
        T: Send,
        SR: Sync,
        SR::Symbol: Sync,
    {
        self.signal
            .clone_runtime_ref()
            .run_detached(|| self.touch());
        self.project_ref()
            .signal
            .update(|value, _| update(&mut value.0.write().unwrap()))
    }

    pub async fn set_async(self: Pin<&Self>, new_value: T)
    where
        T: 'static + Send + Sized,
        SR: Sync,
        SR::Symbol: Sync,
    {
        if needs_drop::<T>() || size_of::<T>() > 0 {
            self.update_async(|value| *value = new_value).await;
        } else {
            // The write is unobservable, so just skip locking.
            self.signal
                .clone_runtime_ref()
                .run_detached(|| self.touch());
            self.project_ref().signal.update_async(|_, _| ()).await;
        }
    }

    pub async fn update_async(self: Pin<&Self>, update: impl 'static + Send + FnOnce(&mut T))
    where
        T: Send,
        SR: Sync,
        SR::Symbol: Sync,
    {
        self.signal
            .clone_runtime_ref()
            .run_detached(|| self.touch());
        self.project_ref()
            .signal
            .update_async(|value, _| update(&mut value.0.write().unwrap()))
            .await
    }

    pub fn set_blocking(&self, new_value: T)
    where
        T: Sized,
    {
        if needs_drop::<T>() || size_of::<T>() > 0 {
            self.update_blocking(|value| *value = new_value)
        } else {
            // The write is unobservable, so just skip locking.
            self.signal.update_blocking(|_, _| ())
        }
    }

    pub fn update_blocking(&self, update: impl FnOnce(&mut T)) {
        self.signal
            .clone_runtime_ref()
            .run_detached(|| self.touch());
        self.signal
            .update_blocking(|value, _| update(&mut value.0.write().unwrap()))
    }

    pub fn get_set_blocking<'a>(
        self: Pin<&'a Self>,
    ) -> (
        impl 'a + Clone + Copy + Unpin + Fn() -> T,
        impl 'a + Clone + Copy + Unpin + Fn(T),
    )
    where
        T: Sync + Send + Copy,
    {
        self.get_clone_set_blocking()
    }

    pub fn get_set<'a>(
        self: Pin<&'a Self>,
    ) -> (
        impl 'a + Clone + Copy + Unpin + Send + Sync + Fn() -> T,
        impl 'a + Clone + Copy + Unpin + Send + Sync + Fn(T),
    )
    where
        T: 'static + Sync + Send + Copy,
        SR: Sync,
        SR::Symbol: Sync,
    {
        self.get_clone_set()
    }

    pub fn get_clone_set_blocking<'a>(
        self: Pin<&'a Self>,
    ) -> (
        impl 'a + Clone + Copy + Unpin + Fn() -> T,
        impl 'a + Clone + Copy + Unpin + Fn(T),
    )
    where
        T: Sync + Send + Clone,
    {
        let this = self.clone();
        (
            move || self.get_clone(),
            move |new_value| this.set_blocking(new_value),
        )
    }

    pub fn get_clone_set<'a>(
        self: Pin<&'a Self>,
    ) -> (
        impl 'a + Clone + Copy + Unpin + Send + Sync + Fn() -> T,
        impl 'a + Clone + Copy + Unpin + Send + Sync + Fn(T),
    )
    where
        T: 'static + Sync + Send + Clone,
        SR: Sync,
        SR::Symbol: Sync,
    {
        let this = self.clone();
        (
            move || self.get_clone(),
            move |new_value| this.set(new_value),
        )
    }

    pub fn into_get_exclusive_set_blocking<'a>(
        self: Pin<&'a Self>,
    ) -> (
        impl 'a + Clone + Copy + Unpin + Fn() -> T,
        impl 'a + Clone + Copy + Unpin + Fn(T),
    )
    where
        Self: 'a,
        T: Send + Copy,
    {
        self.into_get_clone_exclusive_set_blocking()
    }

    pub fn into_get_exclusive_set<'a>(
        self: Pin<&'a Self>,
    ) -> (
        impl 'a + Clone + Copy + Unpin + Send + Sync + Fn() -> T,
        impl 'a + Clone + Copy + Unpin + Send + Sync + Fn(T),
    )
    where
        Self: 'a,
        T: 'static + Send + Copy,
        SR: Sync,
        SR::Symbol: Sync,
    {
        self.into_get_clone_exclusive_set()
    }

    pub fn into_get_clone_exclusive_set_blocking<'a>(
        self: Pin<&'a Self>,
    ) -> (
        impl 'a + Clone + Copy + Unpin + Fn() -> T,
        impl 'a + Clone + Copy + Unpin + Fn(T),
    )
    where
        Self: 'a,
        T: Send + Clone,
    {
        let this = self.clone();
        (
            move || self.get_clone_exclusive(),
            move |new_value| this.set_blocking(new_value),
        )
    }

    pub fn into_get_clone_exclusive_set<'a>(
        self: Pin<&'a Self>,
    ) -> (
        impl 'a + Clone + Copy + Unpin + Send + Sync + Fn() -> T,
        impl 'a + Clone + Copy + Unpin + Send + Sync + Fn(T),
    )
    where
        Self: 'a,
        T: 'static + Send + Clone,
        SR: Sync,
        SR::Symbol: Sync,
    {
        let this = self.clone();
        (
            move || self.get_clone_exclusive(),
            move |new_value| this.set(new_value),
        )
    }
}

impl<T: Send, SR: SignalRuntimeRef> Source<SR> for RawSubject<T, SR> {
    type Output = T;

    fn touch(self: Pin<&Self>) {
        (*self).touch();
    }

    fn get(self: Pin<&Self>) -> Self::Output
    where
        Self::Output: Sync + Copy,
    {
        (*self).get()
    }

    fn get_clone(self: Pin<&Self>) -> Self::Output
    where
        Self::Output: Sync + Clone,
    {
        (*self).get_clone()
    }

    fn get_exclusive(self: Pin<&Self>) -> Self::Output
    where
        Self::Output: Copy,
    {
        (*self).get_exclusive()
    }

    fn get_clone_exclusive(self: Pin<&Self>) -> Self::Output
    where
        Self::Output: Clone,
    {
        (*self).get_clone_exclusive()
    }

    fn read<'a>(self: Pin<&'a Self>) -> Box<dyn 'a + Borrow<Self::Output>>
    where
        Self::Output: Sync,
    {
        Box::new(self.get_ref().read())
    }

    fn read_exclusive<'a>(self: Pin<&'a Self>) -> Box<dyn 'a + Borrow<Self::Output>> {
        Box::new(self.get_ref().read_exclusive())
    }

    fn clone_runtime_ref(&self) -> SR
    where
        SR: Sized,
    {
        self.signal.clone_runtime_ref()
    }
}

impl<T: Send, SR: SignalRuntimeRef> Subscribable<SR> for RawSubject<T, SR> {
    fn subscribe_inherently<'r>(self: Pin<&'r Self>) -> Option<Box<dyn 'r + Borrow<Self::Output>>> {
		//FIXME: This is inefficient.
        if self
            .project_ref()
            .signal
            .subscribe_inherently::<NoCallbacks>(|_, slot| slot.write(()))
            .is_some()
        {
            Some(self.read_exclusive())
        } else {
            None
        }
    }

    fn unsubscribe_inherently(self: Pin<&Self>) -> bool {
        self.project_ref().signal.unsubscribe_inherently()
    }
}
