use std::{
    future::{Future, IntoFuture},
    pin::pin,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

#[track_caller]
pub fn assert_ready<T>(f: impl IntoFuture<Output = T>) -> T {
    match pin!(f.into_future()).poll(&mut Context::from_waker(&waker())) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("Unexpectedly not ready!"),
    }
}

#[track_caller]
pub fn assert_pending<T>(f: impl IntoFuture<Output = T>) {
    match pin!(f.into_future()).poll(&mut Context::from_waker(&waker())) {
        Poll::Ready(_) => panic!("Unexpectedly ready!"),
        Poll::Pending => (),
    }
}

fn waker() -> Waker {
    unsafe { Waker::from_raw(raw_waker()) }
}

fn raw_waker() -> RawWaker {
    RawWaker::new(&(), &RawWakerVTable::new(|_| raw_waker(), drop, drop, drop))
}
