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

pub mod conversions;
mod opaque;

mod signal;
pub use signal::Signal;

pub mod unmanaged;

//TODO: Inter-runtime signals (i.e. takes two signals runtimes as parameters, acts as source for one and dynamic subscriber for the other).

mod signal_arc;
pub use signal_arc::{
	SignalArc, SignalArcDyn, SignalArcDynCell, SignalWeak, SignalWeakDyn, SignalWeakDynCell,
};

mod subscription;
pub use subscription::{Subscription, SubscriptionDyn, SubscriptionDynCell};

mod effect;
pub use effect::Effect;

mod traits;
pub use traits::Guard;

pub use isoprenoid::runtime::{GlobalSignalsRuntime, Propagation, SignalsRuntimeRef};

pub mod prelude {
	//! Unmanaged signal accessors, anonymously.  
	//! You don't need this to use managed signals.

	pub use crate::traits::{UnmanagedSignal as _, UnmanagedSignalCell as _};
}

#[doc(hidden = "macro-only")]
pub mod __ {
	pub use super::unmanaged::{
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
/// use flourish::{shadow_clone, GlobalSignalsRuntime};
///
/// type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
///
/// let a = Signal::cell(1);
/// let b = Signal::cell(2);
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

/// Shadows each reference in place with its [`ToOwned::Owned`].
///
/// This is useful to create cyclic cells:
///
/// ```
/// # {
/// # #![cfg(feature = "global_signals_runtime")] // flourish feature
/// //TODO
/// # }
/// ```
#[macro_export]
macro_rules! shadow_ref_to_owned {
	($ident:ident$(,)?) => {
		// This would warn because of extra parentheses… and it's fewer tokens.
		let $ident = ::std::borrow::ToOwned::to_owned($ident);
	};
    ($($ident:ident),*$(,)?) => {
		let ($($ident),*) = ($(::std::borrow::ToOwned::to_owned($ident)),*);
	};
}
