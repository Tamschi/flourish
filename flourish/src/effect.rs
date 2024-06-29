use std::{marker::PhantomData, pin::Pin};

use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef};

use crate::raw::{new_raw_unsubscribed_effect, pull_effect};

pub type Effect<'a> = EffectSR<'a, GlobalSignalRuntime>;

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
        pull_effect(box_.as_ref());
        Self {
            _raw_effect: box_,
            _phantom: PhantomData,
        }
    }
}
