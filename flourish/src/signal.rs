use std::{borrow::Borrow, marker::PhantomData, mem, pin::Pin, sync::Arc};

use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef};

use crate::{raw::RawComputed, AsSource, Source};

pub type GlobalSignal<T> = Signal<T, GlobalSignalRuntime>;

#[repr(transparent)]
pub struct Signal<T: Send + ?Sized, SR: ?Sized + SignalRuntimeRef = GlobalSignalRuntime> {
    source: Pin<*const dyn Source<SR, Value = T>>,
    _phantom: PhantomData<(Arc<dyn Source<SR, Value = T>>, SR)>,
}

/// TODO
pub struct SignalGuard<'a, T>(PhantomData<&'a T>);

// impl<S: Source<SR, Value = T>, T: Send + ?Sized, SR:?Sized+ SignalRuntimeRef> From<S> for Signal<T, SR> {
//     fn from(value: S) -> Self {
//         Self::new(value)
//     }
// }

impl<T: Send + ?Sized, SR: ?Sized + SignalRuntimeRef> Signal<T, SR> {
    pub fn uncached(source: impl Source<SR, Value = T>) -> Signal<T, SR> {
        Signal {
            source: unsafe {
                mem::transmute::<
                    *const dyn Source<SR, Value = T>,
                    Pin<*const dyn Source<SR, Value = T>>,
                >(Arc::into_raw(Arc::new(source)))
            },
            _phantom: PhantomData,
        }
    }

    pub fn computed(source: impl Send + Source<GlobalSignalRuntime, Value = T>) -> Self
    where
        SR: Default,
        T: Send + Sync + Sized + Clone,
    {
        Self::computed_with_runtime(source, SR::default())
    }

    pub fn computed_with_runtime(
        source: impl Send + Source<GlobalSignalRuntime, Value = T>,
        runtime: SR,
    ) -> Self
    where
        T: Send + Sync + Sized + Clone,
    {
        //TODO: Generalise.
        Self::uncached(RawComputed::with_runtime(
            move || unsafe { Pin::new_unchecked(&source) }.get_clone(),
            runtime,
        ))
    }
}

impl<T: Send + ?Sized, SR: ?Sized + SignalRuntimeRef> Signal<T, SR> {}

impl<T: Send + ?Sized, SR: ?Sized + SignalRuntimeRef> Signal<T, SR> {}

#[repr(transparent)]
pub struct SignalRef<'a, T: 'a + Send + ?Sized, SR: ?Sized + SignalRuntimeRef = GlobalSignalRuntime>
{
    pub(crate) source: Pin<*const (dyn 'a + Source<SR, Value = T>)>,
    _phantom: PhantomData<(&'a (dyn 'a + Source<SR, Value = T>), SR)>,
}

impl<'a, T: Send + ?Sized, SR: ?Sized + SignalRuntimeRef> ToOwned for SignalRef<'a, T, SR> {
    type Owned = Signal<T, SR>;

    fn to_owned(&self) -> Self::Owned {
        unsafe {
            Arc::increment_strong_count(mem::transmute::<
                Pin<*const (dyn 'a + Source<SR, Value = T>)>,
                *const (dyn 'a + Source<SR, Value = T>),
            >(self.source));
        }
        Self::Owned {
            source: unsafe {
                mem::transmute::<
                    Pin<*const (dyn 'a + Source<SR, Value = T>)>,
                    Pin<*const (dyn Source<SR, Value = T>)>,
                >(self.source)
            },
            _phantom: PhantomData,
        }
    }
}

impl<'a, T: Send + ?Sized, SR: ?Sized + SignalRuntimeRef> Borrow<SignalRef<'a, T, SR>>
    for Signal<T, SR>
{
    fn borrow(&self) -> &SignalRef<'a, T, SR> {
        unsafe { &*((self as *const Self).cast()) }
    }
}

impl<'a, T: 'a + Send + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> AsSource<'a, SR>
    for Signal<T, SR>
{
    type Source = dyn Source<SR, Value = T>;

    fn as_source(&self) -> Pin<&Self::Source> {
        unsafe {
            Pin::new_unchecked(&*mem::transmute::<
                Pin<*const dyn Source<SR, Value = T>>,
                *const dyn Source<SR, Value = T>,
            >(self.source))
        }
    }
}

impl<'a, T: 'a + Send + ?Sized, SR: ?Sized + SignalRuntimeRef> AsSource<'a, SR>
    for SignalRef<'a, T, SR>
{
    type Source = dyn Source<SR, Value = T>;

    fn as_source(&self) -> Pin<&Self::Source> {
        unsafe {
            Pin::new_unchecked(&*mem::transmute::<
                Pin<*const dyn Source<SR, Value = T>>,
                *const dyn Source<SR, Value = T>,
            >(self.source))
        }
    }
}
