use std::{
    borrow::Borrow,
    fmt::{self, Debug, Formatter},
    mem::{needs_drop, size_of},
    pin::Pin,
    sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use pin_project::pin_project;
use pollinate::{
    runtime::{CallbackTableTypes, SignalRuntimeRef},
    source::{Callbacks, Source},
};

use crate::utils::conjure_zst;

#[pin_project]
#[repr(transparent)]
pub struct RawProvider<
    T: ?Sized + Send,
    H: Send + FnMut(<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
    SR: SignalRuntimeRef,
> {
    #[pin]
    source: Source<AssertSync<(Mutex<H>, RwLock<T>)>, (), SR>,
}

impl<
        T: ?Sized + Send + Debug,
        H: Send + FnMut(<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus) + Debug,
        SR: SignalRuntimeRef + Debug,
    > Debug for RawProvider<T, H, SR>
where
    SR::Symbol: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawProvider")
            .field("source", &&self.source)
            .finish()
    }
}

/// TODO: Safety.
unsafe impl<
        T: Send,
        H: Send + FnMut(<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
        SR: SignalRuntimeRef + Sync,
    > Sync for RawProvider<T, H, SR>
{
}

struct AssertSync<T: ?Sized>(T);
unsafe impl<T: ?Sized> Sync for AssertSync<T> {}

impl<T: Debug + ?Sized, H: Debug> Debug for AssertSync<(Mutex<H>, RwLock<T>)> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let debug_tuple = &mut f.debug_tuple("AssertSync");
        {
            let maybe_guard = self.0 .1.try_read();
            debug_tuple.field(
                maybe_guard
                    .as_ref()
                    .map_or_else(|_| &"(locked)" as &dyn Debug, |guard| guard),
            );
        }
        {
            let maybe_guard = self.0 .0.try_lock();
            debug_tuple.field(
                maybe_guard
                    .as_ref()
                    .map_or_else(|_| &"(locked)" as &dyn Debug, |guard| guard),
            );
        }
        debug_tuple.finish()
    }
}

struct RawProviderGuard<'a, T: ?Sized>(RwLockReadGuard<'a, T>);
struct RawProviderGuardExclusive<'a, T: ?Sized>(RwLockWriteGuard<'a, T>);

impl<'a, T: ?Sized> Borrow<T> for RawProviderGuard<'a, T> {
    fn borrow(&self) -> &T {
        self.0.borrow()
    }
}

impl<'a, T: ?Sized> Borrow<T> for RawProviderGuardExclusive<'a, T> {
    fn borrow(&self) -> &T {
        self.0.borrow()
    }
}

impl<
        T: ?Sized + Send,
        H: Send + FnMut(<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
        SR: SignalRuntimeRef,
    > RawProvider<T, H, SR>
{
    pub fn new(initial_value: T, handler: H) -> Self
    where
        T: Sized,
        SR: Default,
    {
        Self::with_runtime(initial_value, handler, SR::default())
    }

    pub fn with_runtime(initial_value: T, handler: H, runtime: SR) -> Self
    where
        T: Sized,
    {
        Self {
            source: Source::with_runtime(
                AssertSync((Mutex::new(handler), RwLock::new(initial_value))),
                runtime,
            ),
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
        RawProviderGuard(this.touch().read().unwrap())
    }

    pub fn read_exclusive<'a>(&'a self) -> impl 'a + Borrow<T> {
        let this = &self;
        RawProviderGuardExclusive(this.touch().write().unwrap())
    }

    pub fn get_mut<'a>(&'a mut self) -> &mut T {
        self.source.eager_mut().0 .1.get_mut().unwrap()
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
            &*(&Pin::new_unchecked(&self.source)
                .project_or_init::<E>(|_, slot| slot.write(()))
                .0
                 .0
                 .1 as *const _)
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
            self.source
                .clone_runtime_ref()
                .run_detached(|| self.touch());
            self.project_ref().source.update(|_, _| ());
        }
    }

    pub fn update(self: Pin<&Self>, update: impl 'static + Send + FnOnce(&mut T))
    where
        T: Send,
        SR: Sync,
        SR::Symbol: Sync,
    {
        self.source
            .clone_runtime_ref()
            .run_detached(|| self.touch());
        self.project_ref()
            .source
            .update(|value, _| update(&mut value.0 .1.write().unwrap()))
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
            self.source
                .clone_runtime_ref()
                .run_detached(|| self.touch());
            self.project_ref().source.update_async(|_, _| ()).await;
        }
    }

    pub async fn update_async(self: Pin<&Self>, update: impl 'static + Send + FnOnce(&mut T))
    where
        T: Send,
        SR: Sync,
        SR::Symbol: Sync,
    {
        self.source
            .clone_runtime_ref()
            .run_detached(|| self.touch());
        self.project_ref()
            .source
            .update_async(|value, _| update(&mut value.0 .1.write().unwrap()))
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
            self.source.update_blocking(|_, _| ())
        }
    }

    pub fn update_blocking(&self, update: impl FnOnce(&mut T)) {
        self.source
            .clone_runtime_ref()
            .run_detached(|| self.touch());
        self.source
            .update_blocking(|value, _| update(&mut value.0 .1.write().unwrap()))
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

enum E {}
impl<
        T: ?Sized + Send,
        H: Send + FnMut(<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
        SR: SignalRuntimeRef,
    > Callbacks<AssertSync<(Mutex<H>, RwLock<T>)>, (), SR> for E
{
    const UPDATE: Option<
        fn(
            eager: Pin<&AssertSync<(Mutex<H>, RwLock<T>)>>,
            lazy: Pin<&()>,
        ) -> pollinate::runtime::Update,
    > = None;

    const ON_SUBSCRIBED_CHANGE: Option<
		fn(
			source: Pin<&Source<AssertSync<(Mutex<H>, RwLock<T>)>, (), SR>>,
			eager: Pin<&AssertSync<(Mutex<H>, RwLock<T>)>>,
			lazy: Pin<&()>,
			subscribed: <<SR as SignalRuntimeRef>::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
		),
	> = {
		fn handler<
			T: ?Sized + Send,
			H: Send + FnMut( <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
			SR: SignalRuntimeRef,
		>(_: Pin<&Source<AssertSync<(Mutex<H>, RwLock<T>)>, (), SR>>,eager: Pin<&AssertSync<(Mutex<H>, RwLock<T>)>>, _ :Pin<&()>, status: <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus){
			eager.0.0.lock().unwrap()(status)
		}

		Some(handler::<T,H,SR>)
	};
}

impl<
        T: Send,
        H: Send + FnMut(<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
        SR: SignalRuntimeRef,
    > crate::Source<SR> for RawProvider<T, H, SR>
{
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
        Self::Value: Clone,
    {
        (*self).get_clone_exclusive()
    }

    fn read<'a>(self: Pin<&'a Self>) -> Box<dyn 'a + Borrow<Self::Value>>
    where
        Self::Value: Sync,
    {
        Box::new(self.get_ref().read())
    }

    fn read_exclusive<'a>(self: Pin<&'a Self>) -> Box<dyn 'a + Borrow<Self::Value>> {
        Box::new(self.get_ref().read_exclusive())
    }

    fn clone_runtime_ref(&self) -> SR
    where
        SR: Sized,
    {
        self.source.clone_runtime_ref()
    }
}
