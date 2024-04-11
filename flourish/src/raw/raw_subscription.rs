use std::pin::Pin;

use pin_project::pin_project;

use super::{RawSignal, RawSignalGuard};

#[pin_project]
#[must_use = "Subscriptions are cancelled when dropped."]
pub struct RawSubscription<T: Send, F: Send + ?Sized + FnMut() -> T>(#[pin] RawSignal<T, F>);

//TODO: Implementations
pub struct RawSubscriptionGuard<'a, T>(RawSignalGuard<'a, T>);

impl<T: Send, F: Send + ?Sized + FnMut() -> T> RawSubscription<T, F> {
    //TODO
}

pub fn __new_raw_unsubscribed_subscription<T: Send, F: Send + FnMut() -> T>(
    f: F,
) -> RawSubscription<T, F> {
    RawSubscription(RawSignal::new(f))
}

pub fn __pull_subscription<T: Send, F: Send + FnMut() -> T>(
    subscription: Pin<&RawSubscription<T, F>>,
) {
    subscription.project_ref().0.pull();
}

pub(crate) mod __ {
    pub use super::{__new_raw_unsubscribed_subscription, __pull_subscription};
}

#[macro_export]
macro_rules! subscription {
    {$(let $(mut $(@@ $_mut:ident)?)? $name:ident => $f:expr;)*} => {$(
		let $name = ::std::pin::pin!($crate::__::__new_raw_unsubscribed_subscription(|| $f));
		let $(mut $(@@ $_mut)?)? $name = $name.into_ref();
		$crate::__::__pull_subscription($name);
	)*};
}
