use std::pin::Pin;

use pin_project::pin_project;
use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef};

use super::{RawSignal, RawSignalGuard};

#[pin_project]
#[must_use = "Subscriptions are cancelled when dropped."]
pub struct RawSubscription<
    T: Send,
    F: Send + ?Sized + FnMut() -> T,
    SR: SignalRuntimeRef = GlobalSignalRuntime,
>(#[pin] RawSignal<T, F, SR>);

//TODO: Implementations
pub struct RawSubscriptionGuard<'a, T>(RawSignalGuard<'a, T>);

/// See [rust-lang#98931](https://github.com/rust-lang/rust/issues/98931).
impl<T: Send, F: Send + ?Sized + FnMut() -> T> RawSubscription<T, F> {
    //TODO
}

impl<T: Send, F: Send + ?Sized + FnMut() -> T, SR: SignalRuntimeRef> RawSubscription<T, F, SR> {
    //TODO
}

pub fn __new_raw_unsubscribed_subscription<T: Send, F: Send + FnMut() -> T>(
    f: F,
) -> RawSubscription<T, F> {
    RawSubscription(RawSignal::new(f))
}

pub fn __new_raw_unsubscribed_subscription_with_runtime<
    T: Send,
    F: Send + FnMut() -> T,
    SR: SignalRuntimeRef,
>(
    f: F,
    sr: SR,
) -> RawSubscription<T, F, SR> {
    RawSubscription(RawSignal::with_runtime(f, sr))
}

pub fn __pull_subscription<T: Send, F: Send + FnMut() -> T, SR: SignalRuntimeRef>(
    subscription: Pin<&RawSubscription<T, F, SR>>,
) {
    subscription.project_ref().0.pull();
}

pub(crate) mod __ {
    pub use super::{
        __new_raw_unsubscribed_subscription, __new_raw_unsubscribed_subscription_with_runtime,
        __pull_subscription,
    };
}

#[macro_export]
macro_rules! subscription {
	{$runtime:expr=> $(let $(mut $(@@ $_mut:ident)?)? $name:ident => $f:expr;)*} => {$(
		let $name = ::std::pin::pin!($crate::__::__new_raw_unsubscribed_subscription_with_runtime(|| $f, $runtime));
		let $(mut $(@@ $_mut)?)? $name = $name.into_ref();
		$crate::__::__pull_subscription($name);
	)*};
    {$(let $(mut $(@@ $_mut:ident)?)? $name:ident => $f:expr;)*} => {$(
		let $name = ::std::pin::pin!($crate::__::__new_raw_unsubscribed_subscription(|| $f));
		let $(mut $(@@ $_mut)?)? $name = $name.into_ref();
		$crate::__::__pull_subscription($name);
	)*};
}
