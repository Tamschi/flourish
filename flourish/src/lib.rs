#![warn(clippy::pedantic)]

mod signal;
pub use signal::{Signal, SignalGuard};

mod subject;
pub use subject::{Subject, SubjectGuard};

mod subscription;
pub use subscription::{Subscription, SubscriptionGuard};

#[doc(hidden = "macro-only")]
pub mod __ {
    pub use super::{signal::__::*, subscription::__::*};
}
