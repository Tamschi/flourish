use std::{marker::PhantomData, pin::Pin};

use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef};

use crate::raw::new_raw_unsubscribed_effect;

/// Type inference helper alias for [`EffectSR`] (using [`GlobalSignalRuntime`]).
pub type Effect<'a> = EffectSR<'a, GlobalSignalRuntime>;

/// An [`Effect`] ([`EffectSR`]) subscribes to signal sources just like a [`Subscription`](`crate::Subscription`) does,
/// but instead of caching the value and thereby requiring [`Clone`], it executes side-effects.
///
/// Please note that when an update is received, `drop` consumes the previous value **before** `f` creates the next.
/// *Both* functions are part of the dependency detection scope.
///
/// The specified `drop` function also runs when the [`Effect`] is dropped.
#[must_use = "Effects are cancelled when dropped."]
pub struct EffectSR<'a, SR: 'a + ?Sized + SignalRuntimeRef = GlobalSignalRuntime> {
    _raw_effect: Pin<Box<dyn 'a + DropHandle>>,
    _phantom: PhantomData<SR>,
}

trait DropHandle {}
impl<T: ?Sized> DropHandle for T {}

/// See [rust-lang#98931](https://github.com/rust-lang/rust/issues/98931).
impl<'a, SR: SignalRuntimeRef> EffectSR<'a, SR> {
    pub fn new<T: 'a + Send>(
        f: impl 'a + Send + FnMut() -> T,
        drop: impl 'a + Send + FnMut(T),
    ) -> Self
    where
        SR: Default,
    {
        Self::new_with_runtime(f, drop, SR::default())
    }
    pub fn new_with_runtime<T: 'a + Send>(
        f: impl 'a + Send + FnMut() -> T,
        drop: impl 'a + Send + FnMut(T),
        runtime: SR,
    ) -> Self {
        let box_ = Box::pin(new_raw_unsubscribed_effect(f, drop, runtime));
        box_.as_ref().pull();
        Self {
            _raw_effect: box_,
            _phantom: PhantomData,
        }
    }
}
