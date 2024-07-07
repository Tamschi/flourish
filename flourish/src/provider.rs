use std::{
    borrow::Borrow,
    fmt::Debug,
    mem,
    pin::Pin,
    sync::{Arc, Weak},
};

use pollinate::runtime::{CallbackTableTypes, GlobalSignalRuntime, SignalRuntimeRef};

use crate::{raw::RawProvider, traits::Source, SourcePin};

/// Type inference helper alias for [`ProviderSR`] (using [`GlobalSignalRuntime`]).
pub type Provider<'a, T> = ProviderSR<'a, T, GlobalSignalRuntime>;

/// [`ProviderSR`] functions the same as [`SubjectSR`](`crate::SubjectSR`),
/// except that it is notified of its own subscribed status changes.
///
/// You can use the "`_cyclic`" constructors to easily create self-referential [`ProviderSR`]s:
///
/// ````
/// use flourish::{Provider, WeakProvider};
///
/// let _provider = Provider::new_cyclic(None, |this: WeakProvider<_, _>| move |status| {
///     match status {
///         true => {
///             // You can clone `this` here and then defer the calculation!
///             this.upgrade().unwrap().set(Some(()));
///         }
///         false => this.upgrade().unwrap().set(None),
///     }
/// });
pub struct ProviderSR<'a, T: 'a + ?Sized + Send, SR: 'a + SignalRuntimeRef> {
    provider: Pin<
        Arc<
            RawProvider<
                T,
                Box<
                    dyn 'a
                        + Send
                        + FnMut(<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
                >,
                SR,
            >,
        >,
    >,
}

#[repr(transparent)]
pub struct WeakProvider<'a, T: 'a + ?Sized + Send, SR: 'a + SignalRuntimeRef> {
    provider: Pin<
        Weak<
            RawProvider<
                T,
                Box<
                    dyn 'a
                        + Send
                        + FnMut(<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
                >,
                SR,
            >,
        >,
    >,
}

impl<'a, T: 'a + ?Sized + Send + Debug, SR: 'a + SignalRuntimeRef + Debug> Debug
    for WeakProvider<'a, T, SR>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WeakProvider")
            .field("provider", &self.provider)
            .finish()
    }
}

impl<'a, T: 'a + ?Sized + Send + Clone, SR: 'a + SignalRuntimeRef + Clone> Clone
    for WeakProvider<'a, T, SR>
{
    fn clone(&self) -> Self {
        Self {
            provider: self.provider.clone(),
        }
    }
}

impl<'a, T: 'a + ?Sized + Send, SR: 'a + SignalRuntimeRef> WeakProvider<'a, T, SR> {
    pub fn upgrade(&self) -> Option<ProviderSR<'a, T, SR>> {
        unsafe {
            mem::transmute::<&Pin<Weak<
            RawProvider<
                T,
                Box<
                    dyn 'a
                        + Send
                        + FnMut(<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
                >,
                SR,
            >>>,&Weak<
            RawProvider<
                T,
                Box<
                    dyn 'a
                        + Send
                        + FnMut(<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
                >,
                SR,
            >>>(&self.provider)
        }
        .upgrade()
        .map(|arc| ProviderSR {
            provider: unsafe { Pin::new_unchecked(arc) },
        })
    }
}

impl<'a, T: 'a + ?Sized + Send, SR: 'a + SignalRuntimeRef> Clone for ProviderSR<'a, T, SR> {
    fn clone(&self) -> Self {
        Self {
            provider: self.provider.clone(),
        }
    }
}

impl<'a, T: ?Sized + Debug + Send, SR: SignalRuntimeRef + Debug> Debug for ProviderSR<'a, T, SR>
where
    SR::Symbol: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        //FIXME: This could be more informative.
        f.debug_struct("Provider").finish_non_exhaustive()
    }
}

impl<'a, T: Send, SR: SignalRuntimeRef> ProviderSR<'a, T, SR> {
    pub fn new(
        initial_value: T,
        on_subscribed_status_change_fn_pin: impl 'a
            + Send
            + FnMut(<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
    ) -> Self
    where
        SR: Default,
    {
        Self::with_runtime(
            initial_value,
            on_subscribed_status_change_fn_pin,
            SR::default(),
        )
    }

    pub fn with_runtime(
        initial_value: T,
        on_subscribed_status_change_fn_pin: impl 'a
            + Send
            + FnMut(<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
        runtime: SR,
    ) -> Self
    where
        SR: Default,
    {
        Self {
            provider: Arc::pin(RawProvider::with_runtime(
                initial_value,
                Box::new(on_subscribed_status_change_fn_pin),
                runtime,
            )),
        }
    }

    pub fn new_cyclic<
        HandlerFnPin: 'a + Send + FnMut(<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
    >(
        initial_value: T,
        make_on_subscribed_status_change_fn_pin: impl FnOnce(WeakProvider<'a, T, SR>) -> HandlerFnPin,
    ) -> Self
    where
        SR: Default,
    {
        Self::new_cyclic_with_runtime(
            initial_value,
            make_on_subscribed_status_change_fn_pin,
            SR::default(),
        )
    }

    pub fn new_cyclic_with_runtime<
        HandlerFnPin: 'a + Send + FnMut(<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
    >(
        initial_value: T,
        make_on_subscribed_status_change_fn_pin: impl FnOnce(WeakProvider<'a, T, SR>) -> HandlerFnPin,
        runtime: SR,
    ) -> Self
    where
        SR: Default,
    {
        Self {
            provider: unsafe {
                Pin::new_unchecked(Arc::new_cyclic(|weak| {
                    RawProvider::with_runtime(
						initial_value,
						Box::new(make_on_subscribed_status_change_fn_pin(mem::transmute::<Weak<
							RawProvider<
								T,
								Box<
									dyn 'a
										+ Send
										+ FnMut(<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
								>,
								SR,
							>,
						>, WeakProvider<'a,T,SR>>(weak.clone()))) as Box<_>,
						runtime,
					)
                }))
            },
        }
    }

    pub fn set(&self, new_value: T)
    where
        T: 'static + Send,
        SR: Sync,
        SR::Symbol: Sync,
    {
        self.provider.as_ref().set(new_value)
    }

    pub fn update(&self, update: impl 'static + Send + FnOnce(&mut T))
    where
        SR: Sync,
        SR::Symbol: Sync,
    {
        self.provider.as_ref().update(update)
    }

    pub fn set_blocking(&self, new_value: T) {
        self.provider.set_blocking(new_value)
    }

    pub fn update_blocking(&self, update: impl FnOnce(&mut T)) {
        self.provider.update_blocking(update)
    }

    pub fn into_get_set_blocking(self) -> (impl 'a + Clone + Fn() -> T, impl 'a + Clone + Fn(T))
    where
        Self: 'a,
        T: Sync + Send + Copy,
    {
        self.into_get_clone_set_blocking()
    }

    pub fn into_get_set(
        self,
    ) -> (
        impl 'a + Clone + Send + Sync + Unpin + Fn() -> T,
        impl 'a + Clone + Send + Sync + Unpin + Fn(T),
    )
    where
        Self: 'a,
        T: 'static + Sync + Send + Copy,
        SR: Send + Sync,
        SR::Symbol: Send + Sync,
    {
        self.into_get_clone_set()
    }

    pub fn into_get_clone_set_blocking(
        self,
    ) -> (impl 'a + Clone + Fn() -> T, impl 'a + Clone + Fn(T))
    where
        Self: 'a,
        T: Sync + Send + Clone,
    {
        let this = self.clone();
        (
            move || self.get_clone(),
            move |new_value| this.set_blocking(new_value),
        )
    }

    pub fn into_get_clone_set(
        self,
    ) -> (
        impl 'a + Clone + Send + Sync + Unpin + Fn() -> T,
        impl 'a + Clone + Send + Sync + Unpin + Fn(T),
    )
    where
        Self: 'a,
        T: 'static + Sync + Send + Clone,
        SR: Send + Sync,
        SR::Symbol: Send + Sync,
    {
        let this = self.clone();
        (
            move || self.get_clone(),
            move |new_value| this.set(new_value),
        )
    }

    pub fn into_get_exclusive_set_blocking(
        self,
    ) -> (impl 'a + Clone + Fn() -> T, impl 'a + Clone + Fn(T))
    where
        Self: 'a,
        T: Copy,
    {
        self.into_get_clone_exclusive_set_blocking()
    }

    pub fn into_get_exclusive_set(
        self,
    ) -> (
        impl 'a + Clone + Send + Sync + Fn() -> T,
        impl 'a + Clone + Send + Sync + Fn(T),
    )
    where
        Self: 'a,
        T: 'static + Send + Copy,
        SR: Send + Sync,
        SR::Symbol: Send + Sync,
    {
        self.into_get_clone_exclusive_set()
    }

    pub fn into_get_clone_exclusive_set_blocking(
        self,
    ) -> (impl 'a + Clone + Fn() -> T, impl 'a + Clone + Fn(T))
    where
        Self: 'a,
        T: Clone,
    {
        let this = self.clone();
        (
            move || self.get_clone_exclusive(),
            move |new_value| this.set_blocking(new_value),
        )
    }

    pub fn into_get_clone_exclusive_set(
        self,
    ) -> (
        impl 'a + Clone + Send + Sync + Fn() -> T,
        impl 'a + Clone + Send + Sync + Fn(T),
    )
    where
        Self: 'a,
        T: 'static + Send + Clone,
        SR: Send + Sync,
        SR::Symbol: Send + Sync,
    {
        let this = self.clone();
        (
            move || self.get_clone_exclusive(),
            move |new_value| this.set(new_value),
        )
    }
}

impl<'a, T: Send + Sized + ?Sized, SR: ?Sized + SignalRuntimeRef> SourcePin<SR>
    for ProviderSR<'a, T, SR>
{
    type Output = T;

    fn touch(&self) {
        self.provider.as_ref().touch();
    }

    fn get_clone(&self) -> Self::Output
    where
        Self::Output: Sync + Clone,
    {
        self.provider.as_ref().get_clone()
    }

    fn get_clone_exclusive(&self) -> Self::Output
    where
        Self::Output: Clone,
    {
        self.provider.as_ref().get_clone_exclusive()
    }

    fn read<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Output>>
    where
        Self::Output: 'r + Sync,
    {
        self.provider.as_ref().read()
    }

    fn read_exclusive<'r>(&'r self) -> Box<dyn 'r + Borrow<Self::Output>> {
        self.provider.as_ref().read_exclusive()
    }

    fn clone_runtime_ref(&self) -> SR
    where
        SR: Sized,
    {
        self.provider.as_ref().clone_runtime_ref()
    }
}
