use std::{
	borrow::Borrow,
	fmt::Debug,
	marker::PhantomData,
	mem,
	pin::Pin,
	sync::{Arc, Weak},
};

use isoprenoid::runtime::{CallbackTableTypes, GlobalSignalRuntime, SignalRuntimeRef, Update};

use crate::{
	raw::ReactiveCell,
	traits::{Source, SourceCell, Subscribable},
	SignalRef, SignalSR, SourcePin,
};

/// Type inference helper alias for [`ProviderSR`] (using [`GlobalSignalRuntime`]).
pub type Provider<'a, T> = ProviderSR<'a, T, GlobalSignalRuntime>;

/// [`ProviderSR`] functions the same as [`SignalCellSR`](`crate::SignalCellSR`),
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
///             this.upgrade().unwrap().replace(Some(()));
///         }
///         false => this.upgrade().unwrap().replace(None),
///     }
/// });
pub struct ProviderSR<'a, T: 'a + ?Sized + Send, SR: 'a + SignalRuntimeRef> {
	provider: Pin<
		Arc<
			ReactiveCell<
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
			ReactiveCell<
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
            ReactiveCell<
                T,
                Box<
                    dyn 'a
                        + Send
                        + FnMut(<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus),
                >,
                SR,
            >>>,&Weak<
            ReactiveCell<
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

impl<'a, T: 'a + ?Sized + Debug + Send, SR: 'a + SignalRuntimeRef + Debug> Debug
	for ProviderSR<'a, T, SR>
where
	SR::Symbol: Debug,
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		//FIXME: This could be more informative.
		f.debug_struct("Provider").finish_non_exhaustive()
	}
}

impl<'a, T: 'a + Send, SR: 'a + SignalRuntimeRef> ProviderSR<'a, T, SR> {
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
			provider: Arc::pin(ReactiveCell::with_runtime(
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
					ReactiveCell::with_runtime(
						initial_value,
						Box::new(make_on_subscribed_status_change_fn_pin(mem::transmute::<Weak<
							ReactiveCell<
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

	/// Cheaply borrows this [`Provider`] as [`SignalRef`], which is [`Copy`].
	pub fn as_ref(&self) -> SignalRef<'_, 'a, T, SR> {
		SignalRef {
			source: {
				let ptr =
					Arc::into_raw(unsafe { Pin::into_inner_unchecked(Pin::clone(&self.provider)) });
				unsafe { Arc::decrement_strong_count(ptr) };
				ptr
			},
			_phantom: PhantomData,
		}
	}

	/// Cheaply creates a [`SignalSR`] handle to the managed provider.
	pub fn to_signal(&self) -> SignalSR<'a, T, SR> {
		SignalSR {
			source: Pin::clone(&self.provider) as Pin<Arc<dyn Subscribable<SR, Output = T>>>,
		}
	}

	pub fn read(&'a self) -> impl 'a + Borrow<T>
	where
		T: Sync,
	{
		self.provider.read()
	}

	pub fn read_exclusive(&'a self) -> impl 'a + Borrow<T> {
		self.provider.read_exclusive()
	}

	pub fn change(&self, new_value: T)
	where
		T: 'static + Send + PartialEq,
		SR: Sync,
		SR::Symbol: Sync,
	{
		self.provider.as_ref().change(new_value)
	}

	pub fn replace(&self, new_value: T)
	where
		T: 'static + Send,
		SR: Sync,
		SR::Symbol: Sync,
	{
		self.provider.as_ref().replace(new_value)
	}

	pub fn update(&self, update: impl 'static + Send + FnOnce(&mut T) -> Update)
	where
		SR: Sync,
		SR::Symbol: Sync,
	{
		self.provider.as_ref().update(update)
	}

	pub async fn change_async(&self, new_value: T) -> Result<T, T>
	where
		T: Send + PartialEq,
		SR: Sync,
		SR::Symbol: Sync,
	{
		self.provider.as_ref().change_async(new_value).await
	}

	pub async fn replace_async(&self, new_value: T) -> T
	where
		T: Send,
		SR: Sync,
		SR::Symbol: Sync,
	{
		self.provider.as_ref().replace_async(new_value).await
	}

	pub async fn update_async<U: Send>(
		&self,
		update: impl Send + FnOnce(&mut T) -> (U, Update),
	) -> U
	where
		SR: Sync,
		SR::Symbol: Sync,
	{
		self.provider.as_ref().update_async(update).await
	}

	pub fn change_blocking(&self, new_value: T) -> Result<T, T>
	where
		T: PartialEq,
		SR::Symbol: Sync,
	{
		self.provider.change_blocking(new_value)
	}

	pub fn replace_blocking(&self, new_value: T) -> T
	where
		SR::Symbol: Sync,
	{
		self.provider.replace_blocking(new_value)
	}

	pub fn update_blocking<U>(&self, update: impl FnOnce(&mut T) -> (U, Update)) -> U
	where
		SR::Symbol: Sync,
	{
		self.provider.update_blocking(update)
	}

	pub fn into_signal_and_setter<S>(
		self,
		into_setter: impl FnOnce(Self) -> S,
	) -> (SignalSR<'a, T, SR>, S) {
		(self.to_signal(), into_setter(self))
	}

	pub fn into_getter_and_setter<S, R>(
		self,
		signal_into_getter: impl FnOnce(SignalSR<'a, T, SR>) -> R,
		into_setter: impl FnOnce(Self) -> S,
	) -> (R, S) {
		(signal_into_getter(self.to_signal()), into_setter(self))
	}
}

impl<'a, T: 'a + Send + Sized + ?Sized, SR: 'a + ?Sized + SignalRuntimeRef> SourcePin<SR>
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
