use std::pin::Pin;

use pin_project::pin_project;
use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef};

use super::{new_raw_unsubscribed_attached_effect, raw_attached_effect::RawAttachedEffect};

#[must_use = "Effects are cancelled when dropped."]
#[repr(transparent)]
#[pin_project]
pub struct RawEffect<
    T: Send,
    S: Send + FnMut() -> T,
    D: Send + FnMut(T),
    SR: SignalRuntimeRef = GlobalSignalRuntime,
>(#[pin] RawAttachedEffect<T, S, D, SR>);

#[pin_project]
struct ForceSyncUnpin<T: ?Sized>(#[pin] T);
unsafe impl<T: ?Sized> Sync for ForceSyncUnpin<T> {}

//TODO: Add some associated methods, like not-boxing `read`/`read_exclusive`.
//TODO: Turn some of these functions into methods.

pub fn new_raw_unsubscribed_effect<
    T: Send,
    S: Send + FnMut() -> T,
    D: Send + FnMut(T),
    SR: SignalRuntimeRef,
>(
    source: S,
    drop: D,
    runtime: SR,
) -> RawEffect<T, S, D, SR> {
    RawEffect(new_raw_unsubscribed_attached_effect(source, drop, runtime))
}

impl<T: Send, S: Send + FnMut() -> T, D: Send + FnMut(T), SR: SignalRuntimeRef>
    RawEffect<T, S, D, SR>
{
    pub fn pull(self: Pin<&RawEffect<T, S, D, SR>>) {
        self.project_ref().0.pull()
    }
}
