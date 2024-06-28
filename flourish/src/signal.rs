use std::{
    borrow::Borrow,
    marker::PhantomData,
    mem::{self, forget},
    ops::Deref,
    pin::Pin,
    sync::Arc,
};

use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef, Update};

use crate::{
    raw::{cached, computed, computed_uncached, computed_uncached_mut, merged},
    AsSource, Source,
};

pub type Signal<'a, T> = SignalSR<'a, T, GlobalSignalRuntime>;

#[repr(transparent)]
pub struct SignalSR<
    'a,
    T: 'a + Send + ?Sized,
    SR: 'a + ?Sized + SignalRuntimeRef = GlobalSignalRuntime,
> {
    source: *const (dyn 'a + Send + Source<SR, Value = T>),
    _phantom: PhantomData<Pin<Arc<dyn 'a + Source<SR, Value = T>>>>,
}

/// TODO
pub struct SignalGuard<'a, T>(PhantomData<&'a T>);

unsafe impl<'a, T: Send + ?Sized, SR: ?Sized + SignalRuntimeRef> Send for SignalSR<'a, T, SR> {}
unsafe impl<'a, T: Send + ?Sized, SR: ?Sized + SignalRuntimeRef> Sync for SignalSR<'a, T, SR> {}

impl<'a, T: Send + ?Sized, SR: ?Sized + SignalRuntimeRef> Deref for SignalSR<'a, T, SR> {
    type Target = Pin<&'a (dyn 'a + Send + Source<SR, Value = T>)>;

    fn deref(&self) -> &Self::Target {
        unsafe {
            mem::transmute::<
                &*const (dyn 'a + Send + Source<SR, Value = T>),
                &Pin<&'a (dyn 'a + Send + Source<SR, Value = T>)>,
            >(&self.source)
        }
    }
}

impl<'a, T: Send + ?Sized, SR: ?Sized + SignalRuntimeRef> Clone for SignalSR<'a, T, SR> {
    fn clone(&self) -> Self {
        unsafe { Arc::increment_strong_count(self.source) };
        Self {
            source: self.source,
            _phantom: PhantomData,
        }
    }

    fn clone_from(&mut self, source: &Self) {
        unsafe {
            let source = Arc::from_raw(source.source);
            let mut this = Arc::from_raw(self.source);
            this.clone_from(&source);
            self.source = Arc::into_raw(this);
            forget(source);
        }
    }
}

impl<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> Drop for SignalSR<'a, T, SR> {
    fn drop(&mut self) {
        unsafe { Arc::decrement_strong_count(self.source) }
    }
}

impl<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> SignalSR<'a, T, SR> {
    pub fn new(source: impl 'a + Source<SR, Value = T>) -> Self {
        SignalSR {
            source: Arc::into_raw(Arc::new(source)),
            _phantom: PhantomData,
        }
    }

    pub fn cached(source: impl 'a + Send + Source<SR, Value = T>) -> Self
    where
        T: Send + Sync + Sized + Copy,
    {
        Self::new(cached(source))
    }

    pub fn computed(f: impl 'a + Send + FnMut() -> T) -> Self
    where
        T: Send + Sync + Sized + Clone,
        SR: Default,
    {
        Self::new(computed(f, SR::default()))
    }

    pub fn computed_with_runtime(f: impl 'a + Send + FnMut() -> T, runtime: SR) -> Self
    where
        T: Send + Sync + Sized + Clone,
    {
        Self::new(computed(f, runtime))
    }

    pub fn computed_uncached(f: impl 'a + Send + Sync + Fn() -> T) -> Self
    where
        T: Send + Sync + Sized + Clone,
        SR: Default,
    {
        Self::new(computed_uncached(f, SR::default()))
    }

    pub fn computed_uncached_with_runtime(f: impl 'a + Send + Sync + Fn() -> T, runtime: SR) -> Self
    where
        T: Send + Sync + Sized + Clone,
    {
        Self::new(computed_uncached(f, runtime))
    }

    pub fn computed_uncached_mut(f: impl 'a + Send + FnMut() -> T) -> Self
    where
        T: Send + Sync + Sized + Clone,
        SR: Default,
    {
        Self::new(computed_uncached_mut(f, SR::default()))
    }

    pub fn computed_uncached_mut_with_runtime(f: impl 'a + Send + FnMut() -> T, runtime: SR) -> Self
    where
        T: Send + Sync + Sized + Clone,
    {
        Self::new(computed_uncached_mut(f, runtime))
    }

    pub fn merged(
        source: impl 'a + Source<SR, Value = T>,
        f: impl 'a + Send + FnMut(&mut T, T) -> Update,
    ) -> Self
    where
        T: Clone,
    {
        Self::new(merged(source, f))
    }
}

impl<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> SignalSR<'a, T, SR> {}

impl<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> SignalSR<'a, T, SR> {}

#[repr(transparent)]
#[derive(Debug)]
pub struct SignalRef<
    'r,
    'a,
    T: 'a + Send + ?Sized,
    SR: ?Sized + SignalRuntimeRef = GlobalSignalRuntime,
> {
    pub(crate) source: *const (dyn 'a + Send + Source<SR, Value = T>),
    _phantom: PhantomData<(&'r (dyn 'a + Send + Source<SR, Value = T>), SR)>,
}

impl<'r, 'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> ToOwned
    for SignalRef<'r, 'a, T, SR>
{
    type Owned = SignalSR<'a, T, SR>;

    fn to_owned(&self) -> Self::Owned {
        unsafe {
            Arc::increment_strong_count(self.source);
        }
        Self::Owned {
            source: self.source,
            _phantom: PhantomData,
        }
    }
}

impl<'r, 'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef>
    Borrow<SignalRef<'r, 'a, T, SR>> for SignalSR<'a, T, SR>
{
    fn borrow(&self) -> &SignalRef<'r, 'a, T, SR> {
        unsafe { &*((self as *const Self).cast()) }
    }
}

impl<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> AsSource<'a, SR>
    for SignalSR<'a, T, SR>
{
    type Source = dyn 'a + Source<SR, Value = T>;

    fn as_source(&self) -> Pin<&Self::Source> {
        unsafe { Pin::new_unchecked(&*self.source) }
    }
}

impl<'r, 'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> AsSource<'a, SR>
    for SignalRef<'r, 'a, T, SR>
{
    type Source = dyn 'a + Source<SR, Value = T>;

    fn as_source(&self) -> Pin<&Self::Source> {
        unsafe { Pin::new_unchecked(&*self.source) }
    }
}
