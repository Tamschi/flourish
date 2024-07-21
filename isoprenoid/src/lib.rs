#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![warn(unreachable_pub)]
#![doc = include_str!("../README.md")]
//!
//! # Threading Notes
//!
//! Please note that *none* of the function in this library are guaranteed to produce *any* memory barriers!

#[cfg(all(
	feature = "global_signals_runtime",
	feature = "forbid_global_signals_runtime"
))]
compile_error!("A dependent enabled the `global_signals_runtime` feature, but another forbid this with the `forbid_global_signals_runtime` feature. Please do not enable `global_signals_runtime` in libraries.");

pub mod raw;
pub mod runtime;
pub mod slot;

#[doc = include_str!("../README.md")]
mod readme {}
