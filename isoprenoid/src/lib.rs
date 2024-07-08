#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![warn(unreachable_pub)]
#![doc = include_str!("../README.md")]

pub mod raw;
pub mod runtime;
pub mod slot;

#[doc = include_str!("../README.md")]
mod readme {}
