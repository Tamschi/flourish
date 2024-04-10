use std::pin::Pin;

use pin_project::pin_project;

use crate::{Signal, SignalGuard};

#[pin_project]
#[must_use = "Subscriptions are cancelled when dropped."]
pub struct Subscription<F: Send + FnMut() -> T, T: Send>(#[pin] Signal<F, T>);

//TODO: Implementations
pub struct SubscriptionGuard<'a, T>(SignalGuard<'a, T>);

impl<F: Send + FnMut() -> T, T: Send> Subscription<F, T> {
    pub fn new(f: F) -> Pin<Box<Self>> {
        let this = Box::pin(Self(Signal::new_raw(f)));
        this.as_ref().project_ref().0.pull();
        this
    }

    fn new_raw_unsubscribed(f: F) -> Self {
        Self(Signal::new_raw(f))
    }

    //TODO
}

pub fn new_raw_unsubscribed_subscription<F: Send + FnMut() -> T, T: Send>(
    f: F,
) -> Subscription<F, T> {
    Subscription(Signal::new_raw(f))
}

pub fn pull_subscription<F: Send + FnMut() -> T, T: Send>(subscription: Pin<&Subscription<F, T>>) {
    subscription.project_ref().0.pull();
}

pub(crate) mod __ {
    pub use super::{new_raw_unsubscribed_subscription, pull_subscription};

    #[must_use = "Subscriptions are cancelled when dropped."]
    pub fn must_use_subscription<T>(t: T) -> T {
        t
    }
}

#[macro_export]
macro_rules! subscription {
    {$(let $(mut $(@@ $_mut:ident)?)? $name:ident => $f:expr;)*} => {$(
		let $name = ::std::pin::pin!($crate::__::new_raw_unsubscribed_subscription(|| $f));
		let $(mut $(@@ $_mut)?)? $name = $name.into_ref();
		$crate::__::pull_subscription($name);
	)*};
}
