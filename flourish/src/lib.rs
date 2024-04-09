#![warn(clippy::pedantic)]

mod signal;
pub use signal::{Signal, SignalGuard};

mod subject;
pub use subject::{Subject, SubjectGuard};
