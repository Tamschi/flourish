use std::{marker::PhantomData, pin::Pin};

use isoprenoid_bound::runtime::SignalsRuntimeRef;

use crate::unmanaged::new_raw_unsubscribed_effect;

/// An [`Effect`] subscribes to signal sources just like a [`Subscription`](`crate::Subscription`) does,
/// but instead of exposing the value, its main use is to execute side-effects with cleanup.
///
/// Please note that when the effect is refreshed, `drop_fn_pin` consumes the previous value *before* `fn_pin` creates the next.
/// *Both* closures are part of the dependency detection scope.
///
/// The specified `drop_fn_pin` function also runs when the [`Effect`] is dropped.
#[must_use = "Effects are cancelled when dropped."]
pub struct Effect<'a, SR: 'a + ?Sized + SignalsRuntimeRef> {
	_raw_effect: Pin<Box<dyn 'a + DropHandle>>,
	_phantom: PhantomData<SR>,
}

trait DropHandle {}
impl<T: ?Sized> DropHandle for T {}

impl<'a, SR: SignalsRuntimeRef> Effect<'a, SR> {
	/// A simple effect with computed state and a `drop_fn_pin` cleanup closure that runs first on refresh and drop.
	///
	/// *Both* closures are part of the dependency detection scope.
	pub fn new<T: 'a>(fn_pin: impl 'a + FnMut() -> T, drop_fn_pin: impl 'a + FnMut(T)) -> Self
	where
		SR: Default,
	{
		Self::new_with_runtime(fn_pin, drop_fn_pin, SR::default())
	}

	/// A simple effect with computed state and a `drop_fn_pin` cleanup closure that runs first on refresh and drop.
	///
	/// *Both* closures are part of the dependency detection scope.
	pub fn new_with_runtime<T: 'a>(
		fn_pin: impl 'a + FnMut() -> T,
		drop_fn_pin: impl 'a + FnMut(T),
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
