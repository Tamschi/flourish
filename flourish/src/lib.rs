#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![warn(unreachable_pub)]
// #![warn(clippy::single_call_fn)]
#![cfg_attr(feature = "_doc", doc = include_str!("../README.md"))]
//!
//! # Threading Notes
//!
//! Please note that *none* of the function in this library are guaranteed to produce *any* memory barriers!
//!
//! # Safety Notes
//!
//! [`impl FnMut`](`FnMut`) closures that appear in parameters with "`fn_pin`" in their name are guaranteed to be [pinned](`core::pin`) when called.

mod opaque;

pub mod raw;

//TODO: Inter-runtime signals (i.e. takes two signals runtimes as parameters, acts as source for one and dynamic subscriber for the other).

mod signal_cell;
pub use signal_cell::{SignalCell, SignalCellDyn, SignalCellSR, WeakSignalCell, WeakSignalCellDyn};

mod signal;
pub use signal::{Signal, SignalDyn, SignalRef, SignalRefDyn, SignalSR, WeakSignal, WeakSignalDyn};

mod subscription;
pub use subscription::{
	Subscription, SubscriptionDyn, SubscriptionSR, WeakSubscription, WeakSubscriptionDyn,
};

mod effect;
pub use effect::{Effect, EffectSR};

mod traits;
pub use traits::{Guard, SourceCellPin, SourcePin};

pub use isoprenoid::runtime::{GlobalSignalsRuntime, Propagation, SignalsRuntimeRef};

pub mod prelude {
	//! Flourish's value accessor traits ([`SourcePin`](`crate::traits::SourcePin`),
	//! [`SourceCellPin`](`crate::traits::SourceCellPin`), [`Source`](`crate::traits::Source`)
	//! and [`SourceCell`](`crate::traits::SourceCell`)), anonymously.

	pub use crate::traits::{Source as _, SourceCell as _, SourceCellPin as _, SourcePin as _};
}

#[doc(hidden = "macro-only")]
pub mod __ {
	pub use super::raw::{
		raw_effect::new_raw_unsubscribed_effect,
		raw_subscription::{
			new_raw_unsubscribed_subscription, pin_into_pin_impl_source, pull_new_subscription,
		},
	};
}

/// Shadows each identifier in place with its [`Clone::clone`].
///
/// This is useful to create additional handles:
///
/// ```
/// # {
/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
/// use flourish::{prelude::*, shadow_clone, SignalCell, Signal};
///
/// let a = SignalCell::new(1);
/// let b = SignalCell::new(2);
/// let c = Signal::computed({
///     shadow_clone!(a, b);
///     move || a.get() + b.get()
/// });
///
/// drop((a, b, c));
/// # }
/// ```
#[macro_export]
macro_rules! shadow_clone {
	($ident:ident$(,)?) => {
		// This would warn because of extra parentheses… and it's fewer tokens.
		let $ident = ::std::clone::Clone::clone(&$ident);
	};
    ($($ident:ident),*$(,)?) => {
		let ($($ident),*) = ($(::std::clone::Clone::clone(&$ident)),*);
	};
}
