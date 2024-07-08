use std::{
    borrow::Borrow,
    fmt::{self, Debug, Formatter},
    mem::{self, needs_drop, size_of},
    pin::Pin,
    sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use pin_project::pin_project;
use pollinate::{
    raw::{NoCallbacks, RawSignal},
    runtime::{SignalRuntimeRef, Update},
};

use crate::utils::conjure_zst;

use super::{Source, Subscribable};

#[pin_project]
pub struct RawAnnouncer<T: ?Sized + Send, SR: SignalRuntimeRef> {
    #[pin]
    signal: RawSignal<AssertSync<RwLock<T>>, (), SR>,
}

impl<T: ?Sized + Send + Debug, SR: SignalRuntimeRef + Debug> Debug for RawAnnouncer<T, SR>
where
    SR::Symbol: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawAnnouncer")
            .field("signal", &&self.signal)
            .finish()
    }
}

/// TODO: Safety.
unsafe impl<T: Send + ?Sized, SR: SignalRuntimeRef + Sync> Sync for RawAnnouncer<T, SR> {}

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

struct RawAnnouncerGuard<'a, T: ?Sized>(RwLockReadGuard<'a, T>);
struct RawAnnouncerGuardExclusive<'a, T: ?Sized>(RwLockWriteGuard<'a, T>);

impl<'a, T: ?Sized> Borrow<T> for RawAnnouncerGuard<'a, T> {
    fn borrow(&self) -> &T {
        self.0.borrow()
    }
}

impl<'a, T: ?Sized> Borrow<T> for RawAnnouncerGuardExclusive<'a, T> {
    fn borrow(&self) -> &T {
        self.0.borrow()
    }
}

impl<T: ?Sized + Send, SR: SignalRuntimeRef> RawAnnouncer<T, SR> {
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
        RawAnnouncerGuard(this.touch().read().unwrap())
    }

    pub fn read_exclusive<'a>(&'a self) -> impl 'a + Borrow<T> {
        let this = &self;
        RawAnnouncerGuardExclusive(this.touch().write().unwrap())
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

    pub fn change(self: Pin<&Self>, new_value: T)
    where
        T: 'static + Send + Sized + PartialEq,
        SR: Sync,
        SR::Symbol: Sync,
    {
        if needs_drop::<T>() || size_of::<T>() > 0 {
            self.update(|value| {
                if *value != new_value {
                    *value = new_value;
                    Update::Propagate
                } else {
                    Update::Halt
                }
            });
        } else {
            // The write is unobservable, so just skip locking.
            self.signal
                .clone_runtime_ref()
                .run_detached(|| self.touch());
            self.project_ref().signal.update(|_, _| unsafe {
                //SAFETY: `T` creation and destruction are unobservable and its size is 0.
                if mem::transmute_copy::<(), T>(&()) != mem::transmute_copy::<(), T>(&()) {
                    Update::Propagate
                } else {
                    Update::Halt
                }
            });
        }
    }

    pub fn replace(self: Pin<&Self>, new_value: T)
    where
        T: 'static + Send + Sized,
        SR: Sync,
        SR::Symbol: Sync,
    {
        if needs_drop::<T>() || size_of::<T>() > 0 {
            self.update(|value| {
                *value = new_value;
                Update::Propagate
            });
        } else {
            // The write is unobservable, so just skip locking.
            self.signal
                .clone_runtime_ref()
                .run_detached(|| self.touch());
            self.project_ref().signal.update(|_, _| Update::Propagate);
        }
    }

    pub fn update(self: Pin<&Self>, update: impl 'static + Send + FnOnce(&mut T) -> Update)
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

    pub async fn change_async(self: Pin<&Self>, new_value: T) -> Result<T, T>
    where
        T: Send + Sized + PartialEq,
        SR: Sync,
        SR::Symbol: Sync,
    {
        if size_of::<T>() > 0 {
            self.update_async(|value| {
                if *value != new_value {
                    (Ok(mem::replace(value, new_value)), Update::Propagate)
                } else {
                    (Err(new_value), Update::Halt)
                }
            })
            .await
        } else {
            // The write is unobservable, so just skip locking.
            self.signal
                .clone_runtime_ref()
                .run_detached(|| self.touch());
            self.project_ref()
                .signal
                .update_async(|_, _| unsafe {
                    //SAFETY: `T` creation and destruction are unobservable and its size is 0.
                    if mem::transmute_copy::<(), T>(&()) != mem::transmute_copy::<(), T>(&()) {
                        (Ok(mem::transmute_copy::<(), T>(&())), Update::Propagate)
                    } else {
                        (Err(mem::transmute_copy::<(), T>(&())), Update::Halt)
                    }
                })
                .await
        }
    }

    pub async fn replace_async(self: Pin<&Self>, new_value: T) -> T
    where
        T: Send + Sized,
        SR: Sync,
        SR::Symbol: Sync,
    {
        if size_of::<T>() > 0 {
            self.update_async(|value| (mem::replace(value, new_value), Update::Propagate))
                .await
        } else {
            // The write is unobservable, so just skip locking.
            self.signal
                .clone_runtime_ref()
                .run_detached(|| self.touch());
            self.project_ref()
                .signal
                .update_async(|_, _| (new_value, Update::Propagate))
                .await
        }
    }

    pub async fn update_async<U: Send>(
        self: Pin<&Self>,
        update: impl Send + FnOnce(&mut T) -> (U, Update),
    ) -> U
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

    pub fn change_blocking(&self, new_value: T) -> Result<T, T>
    where
        T: Sized + PartialEq,
    {
        if needs_drop::<T>() || size_of::<T>() > 0 {
            self.update_blocking(|value| {
                if *value != new_value {
                    (Ok(mem::replace(value, new_value)), Update::Propagate)
                } else {
                    (Err(new_value), Update::Halt)
                }
            })
        } else {
            // The write is unobservable, so just skip locking.
            self.signal.update_blocking(|_, _| unsafe {
                //SAFETY: `T` creation and destruction are unobservable and its size is 0.
                if mem::transmute_copy::<(), T>(&()) != mem::transmute_copy::<(), T>(&()) {
                    (Ok(mem::transmute_copy::<(), T>(&())), Update::Propagate)
                } else {
                    (Err(mem::transmute_copy::<(), T>(&())), Update::Halt)
                }
            })
        }
    }

    pub fn replace_blocking(&self, new_value: T) -> T
    where
        T: Sized,
    {
        if size_of::<T>() > 0 {
            self.update_blocking(|value| (mem::replace(value, new_value), Update::Propagate))
        } else {
            // The write is unobservable, so just skip locking.
            self.signal
                .update_blocking(|_, _| (new_value, Update::Propagate))
        }
    }

    pub fn update_blocking<U>(&self, update: impl FnOnce(&mut T) -> (U, Update)) -> U {
        self.signal
            .clone_runtime_ref()
            .run_detached(|| self.touch());
        self.signal
            .update_blocking(|value, _| update(&mut value.0.write().unwrap()))
    }

    pub fn as_source_and_setter<'a, S>(
        self: Pin<&'a Self>,
        as_setter: impl FnOnce(Pin<&'a Self>) -> S,
    ) -> (Pin<&'a impl Source<SR, Output = T>>, S)
    where
        T: Sized,
    {
        (self, as_setter(self))
    }

    pub fn as_getter_and_setter<'a, S, R>(
        self: Pin<&'a Self>,
        source_as_getter: impl FnOnce(Pin<&'a dyn Source<SR, Output = T>>) -> R,
        as_setter: impl FnOnce(Pin<&'a Self>) -> S,
    ) -> (R, S)
    where
        T: Sized,
    {
        (source_as_getter(self), as_setter(self))
    }
}

impl<T: Send + ?Sized, SR: SignalRuntimeRef> Source<SR> for RawAnnouncer<T, SR> {
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

impl<T: Send, SR: SignalRuntimeRef> Subscribable<SR> for RawAnnouncer<T, SR> {
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
