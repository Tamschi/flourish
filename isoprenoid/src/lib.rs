#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![warn(unreachable_pub)]
#![doc = include_str!("../README.md")]
//!
//! # Threading Notes
//!
//! Please note that *none* of the function in this library are guaranteed to produce *any* memory barriers!

pub mod raw;
pub mod runtime;
pub mod slot;

#[doc = include_str!("../README.md")]
mod readme {}
