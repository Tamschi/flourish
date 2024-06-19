use std::{
    borrow::Borrow,
    fmt::{self, Debug, Formatter},
    mem::{needs_drop, size_of},
    ops::Deref,
    pin::Pin,
    sync::{RwLock, RwLockReadGuard},
};

use pin_project::pin_project;
use pollinate::{
    runtime::{GlobalSignalRuntime, SignalRuntimeRef},
    source::{NoCallbacks, Source},
};

use crate::utils::conjure_zst;

#[pin_project]
pub struct RawSubject<T: ?Sized, SR: SignalRuntimeRef = GlobalSignalRuntime> {
    #[pin]
    source: Source<AssertSync<RwLock<T>>, (), SR>,
}

impl<T: ?Sized + Debug, SR: SignalRuntimeRef + Debug> Debug for RawSubject<T, SR>
where
    SR::Symbol: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawSubject")
            .field("source", &&self.source)
            .finish()
    }
}

/// TODO: Safety.
unsafe impl<T, SR: SignalRuntimeRef + Sync> Sync for RawSubject<T, SR> {}

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

pub struct RawSubjectGuard<'a, T: ?Sized>(RwLockReadGuard<'a, T>);

impl<'a, T: ?Sized> Deref for RawSubjectGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.borrow()
    }
}

impl<'a, T: ?Sized> Borrow<T> for RawSubjectGuard<'a, T> {
    fn borrow(&self) -> &T {
        self.0.borrow()
    }
}

/// See [rust-lang#98931](https://github.com/rust-lang/rust/issues/98931).
impl<T> RawSubject<T> {
    pub fn new(initial_value: T) -> Self {
        Self::with_runtime(initial_value, GlobalSignalRuntime)
    }
}

impl<T: ?Sized, SR: SignalRuntimeRef> RawSubject<T, SR> {
    pub fn with_runtime(initial_value: T, sr: SR) -> Self
    where
        T: Sized,
    {
        Self {
            source: Source::with_runtime(AssertSync(RwLock::new(initial_value)), sr),
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
            *self.read()
        }
    }

    pub fn get_clone(&self) -> T
    where
        T: Sync + Clone,
    {
        self.read().clone()
    }

    pub fn read<'a>(&'a self) -> RawSubjectGuard<'a, T>
    where
        T: Sync,
    {
        RawSubjectGuard(self.touch().read().unwrap())
    }

    pub fn get_mut<'a>(&'a mut self) -> &mut T {
        self.source.eager_mut().0.get_mut().unwrap()
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

    pub fn touch(&self) -> &RwLock<T> {
        unsafe {
            // SAFETY: Doesn't defer memory access.
            &*(&Pin::new_unchecked(&self.source)
                .project_or_init::<NoCallbacks>(|_, slot| slot.write(()))
                .0
                 .0 as *const _)
        }
    }

    pub fn set(self: Pin<&Self>, new_value: T)
    where
        T: 'static + Send + Sized,
        SR: 'static + Sync,
        SR::Symbol: Sync,
    {
        if needs_drop::<T>() || size_of::<T>() > 0 {
            self.update(|value| *value = new_value);
        } else {
            // The write is unobservable, so just skip locking.
            self.project_ref().source.update(|_, _| ());
        }
    }

    pub fn update(self: Pin<&Self>, update: impl 'static + Send + FnOnce(&mut T))
    where
        T: Send,
        SR: 'static + Sync,
        SR::Symbol: Sync,
    {
        self.project_ref()
            .source
            .update(|value, _| update(&mut value.0.write().unwrap()))
    }

    pub fn set_blocking(&self, new_value: T)
    where
        T: Sized,
    {
        if needs_drop::<T>() || size_of::<T>() > 0 {
            self.update_blocking(|value| *value = new_value)
        } else {
            // The write is unobservable, so just skip locking.
            self.source.update_blocking(|_, _| ())
        }
    }

    pub fn update_blocking(&self, update: impl FnOnce(&mut T)) {
        self.source
            .update_blocking(|value, _| update(&mut value.0.write().unwrap()))
    }

    pub fn get_set_blocking<'a>(
        self: Pin<&'a Self>,
    ) -> (
        impl 'a + Clone + Copy + Unpin + Fn() -> T,
        impl 'a + Clone + Copy + Unpin + Fn(T),
    )
    where
        T: 'static + Sync + Send + Copy,
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
        SR: 'static + Sync,
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
        T: 'static + Sync + Send + Clone,
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
        SR: 'static + Sync,
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
        T: 'static + Send + Copy,
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
        SR: 'static + Sync,
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
        T: 'static + Send + Clone,
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
        SR: 'static + Sync,
        SR::Symbol: Sync,
    {
        let this = self.clone();
        (
            move || self.get_clone_exclusive(),
            move |new_value| this.set(new_value),
        )
    }
}

#[macro_export]
macro_rules! subject {
	{$runtime:expr=> $(let $(mut $(@@ $_mut:ident)?)? $name:ident := $initial_value:expr;)*} => {$(
		let $name = ::std::pin::pin!($crate::raw::RawSubject::with_runtime($initial_value, $runtime));
		let $(mut $(@@ $_mut)?)? $name = $name.into_ref();
	)*};
    {$(let $(mut $(@@ $_mut:ident)?)? $name:ident := $initial_value:expr;)*} => {$(
		let $name = ::std::pin::pin!($crate::raw::RawSubject::new($initial_value));
		let $(mut $(@@ $_mut)?)? $name = $name.into_ref();
	)*};
}

impl<T: ?Sized, SR: SignalRuntimeRef> crate::Source for RawSubject<T, SR> {
    type Value = T;

    fn touch(self: Pin<&Self>) {
        (*self).touch();
    }

    fn get(self: Pin<&Self>) -> Self::Value
    where
        Self::Value: Sync + Copy,
    {
        (*self).get()
    }

    fn get_clone(self: Pin<&Self>) -> Self::Value
    where
        Self::Value: Sync + Clone,
    {
        (*self).get_clone()
    }

    fn get_exclusive(self: Pin<&Self>) -> Self::Value
    where
        Self::Value: Copy,
    {
        (*self).get_exclusive()
    }

    fn get_clone_exclusive(self: Pin<&Self>) -> Self::Value
    where
        Self::Value: Copy,
    {
        (*self).get_clone_exclusive()
    }

    fn read<'a>(self: Pin<&'a Self>) -> Box<dyn 'a + Borrow<Self::Value>>
    where
        Self::Value: 'a + Sync,
    {
        Box::new(self.get_ref().read())
    }
}
