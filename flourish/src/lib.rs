#![warn(clippy::pedantic)]

pub mod raw;

mod subject;
pub use subject::{Subject, SubjectGuard};

mod signal;
pub use signal::{Signal, SignalGuard};

mod subscription;
pub use subscription::{Subscription, SubscriptionGuard};

mod source;
pub use source::Source;

#[doc(hidden = "macro-only")]
pub mod __ {
    pub use super::raw::__::*;
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
