use std::{marker::PhantomData, pin::Pin};

use isoprenoid::runtime::{GlobalSignalRuntime, SignalRuntimeRef};

use crate::raw::new_raw_unsubscribed_effect;

/// Type inference helper alias for [`EffectSR`] (using [`GlobalSignalRuntime`]).
pub type Effect<'a> = EffectSR<'a, GlobalSignalRuntime>;

/// An [`EffectSR`] subscribes to signal sources just like a [`Subscription`](`crate::Subscription`) does,
/// but instead of caching the value and thereby requiring [`Clone`], it executes side-effects.
///
/// Please note that when an update is received, `drop` consumes the previous value *before* `f` creates the next.
/// *Both* closures are part of the dependency detection scope.
///
/// The specified `drop` function also runs when the [`EffectSR`] is dropped.
#[must_use = "Effects are cancelled when dropped."]
pub struct EffectSR<'a, SR: 'a + ?Sized + SignalRuntimeRef> {
	_raw_effect: Pin<Box<dyn 'a + DropHandle>>,
	_phantom: PhantomData<SR>,
}

trait DropHandle {}
impl<T: ?Sized> DropHandle for T {}

impl<'a, SR: SignalRuntimeRef> EffectSR<'a, SR> {
	/// A simple effect with computed state and a `drop_fn_pin` cleanup closure that runs first on notification and drop.
	///
	/// *Both* closures are part of the dependency detection scope.
	pub fn new<T: 'a + Send>(
		fn_pin: impl 'a + Send + FnMut() -> T,
		drop_fn_pin: impl 'a + Send + FnMut(T),
	) -> Self
	where
		SR: Default,
	{
		Self::new_with_runtime(fn_pin, drop_fn_pin, SR::default())
	}

	/// A simple effect with computed state and a `drop_fn_pin` cleanup closure that runs first on notification and drop.
	///
	/// *Both* closures are part of the dependency detection scope.
	pub fn new_with_runtime<T: 'a + Send>(
		fn_pin: impl 'a + Send + FnMut() -> T,
		drop_fn_pin: impl 'a + Send + FnMut(T),
		runtime: SR,
	) -> Self {
		let box_ = Box::pin(new_raw_unsubscribed_effect(fn_pin, drop_fn_pin, runtime));
		box_.as_ref().pull();
		Self {
			_raw_effect: box_,
			_phantom: PhantomData,
		}
	}
}
