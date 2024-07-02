use std::{borrow::BorrowMut, pin::Pin, sync::Mutex};

use pin_project::{pin_project, pinned_drop};
use pollinate::{
    runtime::{GlobalSignalRuntime, SignalRuntimeRef, Update},
    slot::{Slot, Token},
    source::{Callbacks, Source},
};

#[must_use = "Effects are cancelled when dropped."]
#[repr(transparent)]
#[pin_project(PinnedDrop)]
pub struct RawAttachedEffect<
    T: Send,
    S: Send + FnMut() -> T,
    D: Send + FnMut(T),
    SR: SignalRuntimeRef = GlobalSignalRuntime,
>(#[pin] Source<ForceSyncUnpin<Mutex<(S, D)>>, ForceSyncUnpin<Mutex<Option<T>>>, SR>);

#[pin_project]
struct ForceSyncUnpin<T: ?Sized>(#[pin] T);
unsafe impl<T: ?Sized> Sync for ForceSyncUnpin<T> {}

//TODO: Add some associated methods, like not-boxing `read`/`read_exclusive`.
//TODO: Turn some of these functions into methods.

pub fn new_raw_unsubscribed_attached_effect<
    T: Send,
    S: Send + FnMut() -> T,
    D: Send + FnMut(T),
    SR: SignalRuntimeRef,
>(
    source: S,
    drop: D,
    runtime: SR,
) -> RawAttachedEffect<T, S, D, SR> {
    RawAttachedEffect(Source::with_runtime(
        ForceSyncUnpin((source, drop).into()),
        runtime,
    ))
}

#[pinned_drop]
impl<T: Send, S: Send + FnMut() -> T, D: Send + FnMut(T), SR: SignalRuntimeRef> PinnedDrop
    for RawAttachedEffect<T, S, D, SR>
{
    fn drop(self: Pin<&mut Self>) {
        let source = self.project().0;
        source.stop_and(|eager, lazy| {
            let drop = &mut eager.0.try_lock().unwrap().1;
            lazy.0
                .try_lock()
                .unwrap()
                .borrow_mut()
                .take()
                .and_then(|value| Some(drop(value)));
        });
    }
}

enum E {}
impl<T: Send, S: Send + FnMut() -> T, D: Send + FnMut(T), SR: SignalRuntimeRef>
    Callbacks<ForceSyncUnpin<Mutex<(S, D)>>, ForceSyncUnpin<Mutex<Option<T>>>, SR> for E
{
    const UPDATE: Option<
        unsafe fn(
            eager: Pin<&ForceSyncUnpin<Mutex<(S, D)>>>,
            lazy: Pin<&ForceSyncUnpin<Mutex<Option<T>>>>,
        ) -> pollinate::runtime::Update,
    > = {
        unsafe fn eval<T: Send, S: Send + FnMut() -> T, D: Send + FnMut(T)>(
            source: Pin<&ForceSyncUnpin<Mutex<(S, D)>>>,
            cache: Pin<&ForceSyncUnpin<Mutex<Option<T>>>>,
        ) -> Update {
            let (source, drop) = &mut *source.0.lock().expect("unreachable");
            let cache = &mut *cache.0.lock().expect("unreachable");
            cache.take().map(drop);
            *cache = Some(source());
            Update::Halt
        }
        Some(eval)
    };

    const ON_SUBSCRIBED_CHANGE: Option<
        unsafe fn(
            eager: Pin<&ForceSyncUnpin<Mutex<(S, D)>>>,
            lazy: Pin<&ForceSyncUnpin<Mutex<Option<T>>>>,
            subscribed: bool,
        ),
    > = None;
}

impl<T: Send, S: Send + FnMut() -> T, D: Send + FnMut(T), SR: SignalRuntimeRef>
    RawAttachedEffect<T, S, D, SR>
{
    /// # Safety
    ///
    /// These are the only functions that access `cache`.
    /// Externally synchronised through guarantees on [`pollinate::init`].
    unsafe fn init<'a>(
        source: Pin<&'a ForceSyncUnpin<Mutex<(S, D)>>>,
        cache: Slot<'a, ForceSyncUnpin<Mutex<Option<T>>>>,
    ) -> Token<'a> {
        cache.write(ForceSyncUnpin(
            Some(source.project_ref().0.lock().expect("unreachable").0()).into(),
        ))
    }

    pub fn pull(self: Pin<&RawAttachedEffect<T, S, D, SR>>) {
        self.0.clone_runtime_ref().run_detached(|| unsafe {
            Pin::new_unchecked(&self.0).pull_or_init::<E>(|source, cache| {
                RawAttachedEffect::<T, S, D, SR>::init(source, cache)
            });
        })
    }
}
