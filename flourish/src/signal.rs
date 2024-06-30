use std::{borrow::Borrow, marker::PhantomData, mem, ops::Deref, pin::Pin, sync::Arc};

use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef, Update};

use crate::{
    raw::{computed, computed_uncached, computed_uncached_mut, folded, merged},
    Source, SourcePin,
};

pub type Signal<'a, T> = SignalSR<'a, T, GlobalSignalRuntime>;

/// A largely type-erased signal handle that is all of [`Clone`], [`Send`], [`Sync`] and [`Unpin`].
///
/// You can [`Borrow`] this handle into a reference without indirection that is [`ToOwned`].
///
/// This type is [`Deref`] towards its pinned `dyn `[`Source`]`<SR, Value = T>`, through which you can retrieve its current value.
///
/// Signals are not evaluated unless they are subscribed-to.
#[derive(Clone)]
pub struct SignalSR<
    'a,
    T: 'a + Send + ?Sized,
    SR: 'a + ?Sized + SignalRuntimeRef = GlobalSignalRuntime,
> {
    source: Pin<Arc<dyn 'a + Source<SR, Value = T>>>,
}

unsafe impl<'a, T: Send + ?Sized, SR: ?Sized + SignalRuntimeRef> Send for SignalSR<'a, T, SR> {}
unsafe impl<'a, T: Send + ?Sized, SR: ?Sized + SignalRuntimeRef> Sync for SignalSR<'a, T, SR> {}

impl<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> SignalSR<'a, T, SR> {
    /// Creates a new [`SignalSR`] from the provided raw [`Source`].
    pub fn new(source: impl 'a + Source<SR, Value = T>) -> Self {
        SignalSR {
            source: Arc::pin(source),
        }
    }

    /// Creates a new [`SignalSR`] from the provided [`Send`]` + `[`FnMut`]`() -> T`.
    ///
    /// This pins the provided closure. The resulting `T` is cached.
    pub fn computed(f: impl 'a + Send + FnMut() -> T) -> Self
    where
        T: Sized,
        SR: Default,
    {
        Self::new(computed(f, SR::default()))
    }

    /// Creates a new [`SignalSR`] from the provided [`Send`]` + `[`FnMut`]`() -> T` and [`SignalRuntimeRef`].
    ///
    /// This pins the provided closure. The resulting `T` is cached.
    pub fn computed_with_runtime(f: impl 'a + Send + FnMut() -> T, runtime: SR) -> Self
    where
        T: Sized,
    {
        Self::new(computed(f, runtime))
    }

    /// Creates a new [`SignalSR`] from the provided [`Send`]` + `[`Sync`]` + `[`Fn`]`() -> T`.
    ///
    /// This pins the provided closure. The resulting `T` is **not** cached, so the closure runs each time the value is retrieved. This may lead to congestion.
    pub fn computed_uncached(f: impl 'a + Send + Sync + Fn() -> T) -> Self
    where
        T: Sized,
        SR: Default,
    {
        Self::new(computed_uncached(f, SR::default()))
    }

    /// Creates a new [`SignalSR`] from the provided [`Send`]` + `[`Sync`]` + `[`Fn`]`() -> T` and [`SignalRuntimeRef`].
    ///
    /// This pins the provided closure. The resulting `T` is **not** cached, so the closure runs each time the value is retrieved. This may lead to congestion.
    pub fn computed_uncached_with_runtime(f: impl 'a + Send + Sync + Fn() -> T, runtime: SR) -> Self
    where
        T: Sized,
    {
        Self::new(computed_uncached(f, runtime))
    }

    /// Creates a new [`SignalSR`] from the provided [`Send`]` + `[`FnMut`]`() -> T`.
    ///
    /// This pins the provided closure. The resulting `T` is **not** cached, so the closure runs **exclusively** each time the value is retrieved. This may lead to (more) congestion.
    pub fn computed_uncached_mut(f: impl 'a + Send + FnMut() -> T) -> Self
    where
        T: Sized,
        SR: Default,
    {
        Self::new(computed_uncached_mut(f, SR::default()))
    }

    /// Creates a new [`SignalSR`] from the provided [`Send`]` + `[`FnMut`]`() -> T` and [`SignalRuntimeRef`].
    ///
    /// This pins the provided closure. The resulting `T` is **not** cached, so the closure runs **exclusively** each time the value is retrieved. This may lead to (more) congestion.
    pub fn computed_uncached_mut_with_runtime(f: impl 'a + Send + FnMut() -> T, runtime: SR) -> Self
    where
        T: Sized,
    {
        Self::new(computed_uncached_mut(f, runtime))
    }

    /// This is a convenience method. See [`folded`](`folded()`).
    pub fn folded(init: T, f: impl 'a + Send + FnMut(&mut T) -> Update) -> Self
    where
        T: Sized,
        SR: Default,
    {
        Self::new(folded(init, f, SR::default()))
    }

    /// This is a convenience method. See [`folded`](`folded()`).
    pub fn folded_with_runtime(
        init: T,
        f: impl 'a + Send + FnMut(&mut T) -> Update,
        runtime: SR,
    ) -> Self
    where
        T: Sized,
    {
        Self::new(folded(init, f, runtime))
    }

    /// This is a convenience method. See [`merged`](`merged()`).
    pub fn merged(
        select: impl 'a + Send + FnMut() -> T,
        merge: impl 'a + Send + FnMut(&mut T, T) -> Update,
    ) -> Self
    where
        T: Sized,
        SR: Default,
    {
        Self::new(merged(select, merge, SR::default()))
    }

    /// This is a convenience method. See [`merged`](`merged()`).
    pub fn merged_with_runtime(
        select: impl 'a + Send + FnMut() -> T,
        merge: impl 'a + Send + FnMut(&mut T, T) -> Update,
        runtime: SR,
    ) -> Self
    where
        T: Sized,
    {
        Self::new(merged(select, merge, runtime))
    }
}

impl<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> SourcePin<SR>
    for SignalSR<'a, T, SR>
{
    type Value = T;

    fn touch(&self) {
        self.source.as_ref().touch()
    }

    fn get_clone(&self) -> Self::Value
    where
        Self::Value: Sync + Clone,
    {
        self.source.as_ref().get_clone()
    }

    fn get_clone_exclusive(&self) -> Self::Value
    where
        Self::Value: Clone,
    {
        self.source.as_ref().get_clone_exclusive()
    }

    fn read<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Value>>
    where
        Self::Value: 'r + Sync,
    {
        self.source.as_ref().read()
    }

    fn read_exclusive<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Value>> {
        self.source.as_ref().read_exclusive()
    }

    fn clone_runtime_ref(&self) -> SR
    where
        SR: Sized,
    {
        self.source.as_ref().clone_runtime_ref()
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct SignalRef<
    'r,
    'a,
    T: 'a + Send + ?Sized,
    SR: ?Sized + SignalRuntimeRef = GlobalSignalRuntime,
> {
    pub(crate) source: *const (dyn 'a + Source<SR, Value = T>),
    _phantom: PhantomData<(&'r (dyn 'a + Source<SR, Value = T>), SR)>,
}

impl<'r, 'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> ToOwned
    for SignalRef<'r, 'a, T, SR>
{
    type Owned = SignalSR<'a, T, SR>;

    fn to_owned(&self) -> Self::Owned {
        Self::Owned {
            source: unsafe {
                Arc::increment_strong_count(self.source);
                Pin::new_unchecked(Arc::from_raw(self.source))
            },
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

impl<'r, 'a, T: Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> Deref
    for SignalRef<'r, 'a, T, SR>
{
    type Target = Pin<&'r (dyn 'a + Source<SR, Value = T>)>;

    fn deref(&self) -> &Self::Target {
        unsafe {
            mem::transmute::<
                &*const (dyn 'a + Source<SR, Value = T>),
                &Pin<&'r (dyn 'a + Source<SR, Value = T>)>,
            >(&self.source)
        }
    }
}
