#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![warn(unreachable_pub)]
#![cfg_attr(feature = "_doc", doc = include_str!("../README.md"))]

#[cfg(all(
	feature = "local_signals_runtime",
	feature = "forbid_global_signals_runtime"
))]
compile_error!("A dependent enabled the `local_signals_runtime` feature, but another forbid this with the `forbid_global_signals_runtime` feature. Please do not enable `local_signals_runtime` in libraries.");

pub mod raw;
pub mod runtime;
pub mod slot;
