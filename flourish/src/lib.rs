#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![warn(unreachable_pub)]
// #![warn(clippy::single_call_fn)]
#![doc = include_str!("../README.md")]
//!
//! # Safety Notes
//!
//! [`impl FnMut`](`FnMut`) closures that appear in parameters with "`fn_pin`" in their name are guaranteed to be [pinned](`core::pin`) when called.

pub mod raw;

//TODO: Inter-runtime signals (i.e. takes two signal runtimes as parameters, acts as source for one and dynamic subscriber for the other).

mod subject;
pub use subject::{Subject, SubjectSR};

mod provider;
pub use provider::{Provider, ProviderSR, WeakProvider};

mod signal;
pub use signal::{Signal, SignalRef, SignalSR};

mod subscription;
pub use subscription::{Subscription, SubscriptionSR};

mod effect;
pub use effect::{Effect, EffectSR};

mod traits;
pub use traits::SourcePin;

pub use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef, Update};

#[doc(hidden = "macro-only")]
pub mod __ {
    pub use super::raw::raw_effect::new_raw_unsubscribed_effect;
    pub use super::raw::raw_subscription::{
        new_raw_unsubscribed_subscription, pin_into_pin_impl_source, pull_subscription,
    };
}

/// Shadows each identifier in place with its [`Clone::clone`].
///
/// This is useful to create additional handles:
///
/// ```
/// use flourish::{shadow_clone, Subject, Signal, SourcePin as _};
///
/// let a = Subject::new(1);
/// let b = Subject::new(2);
/// let c = Signal::computed({
///     shadow_clone!(a, b);
///     move || a.get() + b.get()
/// });
///
/// drop((a, b, c));
/// ```
#[macro_export]
macro_rules! shadow_clone {
    ($($ident:ident),*$(,)?) => {
		let ($($ident),*) = ($(::std::clone::Clone::clone(&$ident)),*);
	};
}

mod utils;

//TODO: Consistent formatting setup (with tabs).
