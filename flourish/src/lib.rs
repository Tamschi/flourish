#![warn(clippy::pedantic)]

pub mod raw;

//TODO: Inter-runtime signals (i.e. takes two signal runtimes as parameters, acts as source for one and dynamic subscriber for the other).

mod subject;
pub use subject::{Subject, SubjectGuard};

mod subscription;
pub use subscription::{Subscription, SubscriptionGuard};

mod source;
pub use source::{AsSource, Source};

mod signal;
pub use signal::{Signal, SignalGuard, SignalSR};

pub use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef, Update};

#[doc(hidden = "macro-only")]
pub mod __ {
    pub use super::raw::raw_subscription::{
        new_raw_unsubscribed_subscription_with_runtime, pull_subscription,pin_into_pin_impl_source
    };
}

#[macro_export]
macro_rules! shadow_clone {
    ($($ident:ident),*$(,)?) => {
		let ($($ident),*) = ($(::std::clone::Clone::clone(&$ident)),*);
	};
}

mod utils;

#[doc = include_str!("../README.md")]
mod readme {}
