#![warn(clippy::pedantic)]
#![doc = include_str!("../README.md")]

pub mod raw;

//TODO: Inter-runtime signals (i.e. takes two signal runtimes as parameters, acts as source for one and dynamic subscriber for the other).

mod subject;
pub use subject::Subject;

mod subscription;
pub use subscription::{Subscription, SubscriptionSR};

mod effect;
pub use effect::{Effect, EffectSR};

mod source;
pub use source::{Source, SourcePin};

mod signal;
pub use signal::{Signal, SignalRef, SignalSR};

pub use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef, Update};

#[doc(hidden = "macro-only")]
pub mod __ {
    pub use super::raw::raw_effect::new_raw_unsubscribed_effect;
    pub use super::raw::raw_subscription::{
        new_raw_unsubscribed_subscription, pin_into_pin_impl_source, pull_subscription,
    };
}

#[macro_export]
macro_rules! shadow_clone {
    ($($ident:ident),*$(,)?) => {
		let ($($ident),*) = ($(::std::clone::Clone::clone(&$ident)),*);
	};
}

mod utils;

//TODO: IntoFuture for Signal<Option<T>>.
