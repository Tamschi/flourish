use std::{
    borrow::Borrow,
    cell::Cell,
    marker::{PhantomData, Send},
    mem::MaybeUninit,
    ops::Deref,
    pin::Pin,
    ptr::NonNull,
    sync::{Arc, RwLock, RwLockReadGuard},
};

use pin_project::pin_project;
use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef};
use sptr::{from_exposed_addr, Strict};

use crate::{raw::RawSignal, Source};

#[derive(Debug)]
pub struct Signal<T: Send + ?Sized, SR: SignalRuntimeRef = GlobalSignalRuntime>(
    /// In theory it's possible to store an invalid `*const T` here,
    /// in order to store pointer metadata, which would allow working with unsized type, maybe.
    NonNull<SignalHeader<T, SR>>,
);

unsafe impl<T: Send + ?Sized, SR: SignalRuntimeRef + Send> Send for Signal<T, SR> {}
unsafe impl<T: Send + ?Sized, SR: SignalRuntimeRef + Sync> Sync for Signal<T, SR> {}

impl<T: Send + ?Sized, SR: SignalRuntimeRef + Clone> Clone for Signal<T, SR> {
    fn clone(&self) -> Self {
        Self(unsafe {
            // SAFETY: `Arc` uses enough `repr(C)` to increment the reference without the actual type.
            let from_raw = Arc::from_raw(self.0.as_ptr().cast_const());
            let cloned = from_raw.clone();
            let _ = Arc::into_raw(from_raw);
            NonNull::new_unchecked(Arc::into_raw(cloned).cast_mut())
        })
    }
}

impl<T: Send + ?Sized, SR: SignalRuntimeRef> Drop for Signal<T, SR> {
    fn drop(&mut self) {
        // I think this is synchronised by dropping the Arc.
        let address = unsafe { self.0.as_ptr().read() }.0;
        unsafe { address.as_ref().drop_arc() };
    }
}

pub struct SignalGuard<'a, T: ?Sized>(RwLockReadGuard<'a, T>);

impl<'a, T: ?Sized> Deref for SignalGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl<'a, T: ?Sized> Borrow<T> for SignalGuard<'a, T> {
    fn borrow(&self) -> &T {
        self.0.borrow()
    }
}

/// See [rust-lang#98931](https://github.com/rust-lang/rust/issues/98931).
impl<T: Send + ?Sized> Signal<T> {
    pub fn new<F: Send + FnMut() -> T>(f: F) -> Self
    where
        T: Sized,
    {
        Self::with_runtime(f, GlobalSignalRuntime)
    }
}

impl<T: Send + ?Sized, SR: SignalRuntimeRef> Signal<T, SR> {
    pub fn with_runtime<F: Send + FnMut() -> T>(f: F, sr: SR) -> Self
    where
        T: Sized,
    {
        let arc = Arc::new(SignalDataFull {
            anchor: SignalDataAnchor(PhantomData),
            header: Cell::new(MaybeUninit::uninit()),
            signal: RawSignal::with_runtime(f, sr),
        });
        unsafe {
            arc.header
                .set(MaybeUninit::new(SignalHeader(NonNull::new_unchecked(
                    &arc.anchor as &dyn SignalDataAddress<T, SR> as *const _ as *mut _,
                ))))
        };

        Self(unsafe {
            let signal_data_full = Arc::into_raw(arc);
            let _ = Strict::expose_addr(signal_data_full);
            NonNull::new_unchecked(signal_data_full.cast_mut().cast())
        })
    }

    pub fn get(&self) -> T
    where
        T: Sync + Copy,
    {
        *self.read()
    }

    pub fn get_clone(&self) -> T
    where
        T: Sync + Clone,
    {
        self.read().clone()
    }

    pub fn read<'a>(&'a self) -> SignalGuard<'a, T>
    where
        T: Sync,
    {
        SignalGuard(self.touch().read().unwrap())
    }

    pub fn get_exclusive(&self) -> T
    where
        T: Copy,
    {
        self.get_clone_exclusive()
    }

    pub fn get_clone_exclusive(&self) -> T
    where
        T: Clone,
    {
        self.touch().write().unwrap().clone()
    }

    fn touch(&self) -> &RwLock<T> {
        let address = unsafe { self.0.as_ptr().read() }.0;
        unsafe { address.as_ref().touch() }
    }

    pub(crate) fn pull(&self) -> &RwLock<T> {
        let address = unsafe { self.0.as_ptr().read() }.0;
        unsafe { address.as_ref().pull() }
    }
}

/// FIXME: Once pointer-metadata is available, shrink this.
#[derive(Debug, Clone, Copy)]
struct SignalHeader<T: Send + ?Sized, SR: SignalRuntimeRef>(NonNull<dyn SignalDataAddress<T, SR>>);

trait SignalDataAddress<T: Send + ?Sized, SR: SignalRuntimeRef> {
    unsafe fn drop_arc(&self);
    fn touch(&self) -> &RwLock<T>;
    fn pull(&self) -> &RwLock<T>;
}

#[pin_project]
#[repr(C)]
struct SignalDataFull<T: Send, F: Send + ?Sized + FnMut() -> T, SR: SignalRuntimeRef> {
    anchor: SignalDataAnchor<T, F, SR>,
    header: Cell<MaybeUninit<SignalHeader<T, SR>>>,
    #[pin]
    signal: RawSignal<T, F, SR>,
}

/// MUST BE A ZST
struct SignalDataAnchor<T: Send, F: Send + ?Sized + FnMut() -> T, SR: SignalRuntimeRef>(
    PhantomData<*const SignalDataFull<T, F, SR>>,
);

impl<T: Send, F: Send + FnMut() -> T, SR: SignalRuntimeRef> SignalDataAddress<T, SR>
    for SignalDataAnchor<T, F, SR>
{
    /// # Safety
    ///
    /// `Self` is a ZST, so it's not actually dereferenced.
    ///
    unsafe fn drop_arc(&self) {
        drop(Arc::<SignalDataFull<T, F, SR>>::from_raw(
            from_exposed_addr(Strict::addr(self as *const Self)),
        ))
    }

    fn touch(&self) -> &RwLock<T> {
        unsafe {
            Pin::new_unchecked(&*from_exposed_addr::<SignalDataFull<T, F, SR>>(
                Strict::addr(self as *const Self),
            ))
        }
        .project_ref()
        .signal
        .touch()
    }

    fn pull(&self) -> &RwLock<T> {
        unsafe {
            Pin::new_unchecked(&*from_exposed_addr::<SignalDataFull<T, F, SR>>(
                Strict::addr(self as *const Self),
            ))
        }
        .project_ref()
        .signal
        .pull()
    }
}

impl<T: ?Sized + Send, SR: SignalRuntimeRef> Source for Signal<T, SR> {
    type Value = T;

    fn touch(&self) {
        self.touch();
    }

    fn get(&self) -> Self::Value
    where
        Self::Value: Sync + Copy,
    {
        self.get()
    }

    fn get_clone(&self) -> Self::Value
    where
        Self::Value: Sync + Clone,
    {
        self.get_clone()
    }

    fn get_exclusive(&self) -> Self::Value
    where
        Self::Value: Copy,
    {
        self.get_exclusive()
    }

    fn get_clone_exclusive(&self) -> Self::Value
    where
        Self::Value: Copy,
    {
        self.get_clone_exclusive()
    }

    fn read(&self) -> Box<dyn '_ + Borrow<Self::Value>>
    where
        Self::Value: Sync,
    {
        Box::new(self.read())
    }
}
