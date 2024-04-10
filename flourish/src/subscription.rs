use crate::{Signal, SignalGuard};

#[must_use = "Subscriptions are cancelled when dropped."]
pub struct Subscription<T: Send + ?Sized>(Signal<T>);

//TODO: Implementations
pub struct SubscriptionGuard<'a, T>(SignalGuard<'a, T>);

impl<T: Send + ?Sized> Subscription<T> {
    pub fn new<F: Send + FnMut() -> T>(f: F) -> Self
    where
        T: Sized,
    {
        let this = Self(Signal::new(f));
        this.0.pull();
        this
    }
}
