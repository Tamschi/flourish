use std::{
    borrow::Borrow,
    cell::Cell,
    marker::PhantomData,
    mem::MaybeUninit,
    ops::Deref,
    ptr::NonNull,
    sync::{RwLock, RwLockReadGuard},
};

use pin_project::pin_project;
use servo_arc::Arc;

use crate::raw::RawSignal;

#[derive(Debug)]
pub struct Signal<T: Send + ?Sized>(
    /// In theory it's possible to store an invalid `*const T` here,
    /// in order to store pointer metadata, which would allow working with unsized type, maybe.
    NonNull<SignalHeader<T>>,
);

unsafe impl<T: Send + ?Sized> Send for Signal<T> {}
unsafe impl<T: Send + ?Sized> Sync for Signal<T> {}

impl<T: Send + ?Sized> Clone for Signal<T> {
    fn clone(&self) -> Self {
        Self(unsafe {
            // SAFETY: `Arc` uses enough `repr(C)` to increment the reference without the actual type.
            NonNull::new_unchecked(
                Arc::into_raw(Arc::from_raw_addrefed(self.0.as_ptr().cast_const())).cast_mut(),
            )
        })
    }
}

impl<T: Send + ?Sized> Drop for Signal<T> {
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

impl<T: Send + ?Sized> Signal<T> {
    pub fn new<F: Send + FnMut() -> T>(f: F) -> Self
    where
        T: Sized,
    {
        let arc = Arc::new(SignalDataFull {
            anchor: SignalDataAnchor(PhantomData),
            header: Cell::new(MaybeUninit::uninit()),
            signal: RawSignal::new(f),
        });
        unsafe {
            arc.header
                .set(MaybeUninit::new(SignalHeader(NonNull::new_unchecked(
                    &arc.anchor as &dyn AtSignalDataAddress<T> as *const _ as *mut _,
                ))))
        };
        Self(unsafe { NonNull::new_unchecked(Arc::into_raw(arc).cast_mut().cast()) })
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
struct SignalHeader<T: Send + ?Sized>(NonNull<dyn AtSignalDataAddress<T>>);

trait AtSignalDataAddress<T: Send + ?Sized> {
    unsafe fn drop_arc(&self);
    fn touch(&self) -> &RwLock<T>;
    fn pull(&self) -> &RwLock<T>;
}

#[pin_project]
#[repr(C)]
struct SignalDataFull<T: Send, F: Send + ?Sized + FnMut() -> T> {
    anchor: SignalDataAnchor<T, F>,
    header: Cell<MaybeUninit<SignalHeader<T>>>,
    #[pin]
    signal: RawSignal<T, F>,
}

/// MUST BE A ZST
struct SignalDataAnchor<T: Send, F: Send + ?Sized + FnMut() -> T>(
    PhantomData<*const SignalDataFull<T, F>>,
);

/// TODO: This definitely has wrong provenance.
impl<T: Send, F: Send + FnMut() -> T> AtSignalDataAddress<T> for SignalDataAnchor<T, F> {
    /// # Safety
    ///
    /// `Self` is a ZST, so it's not actually dereferenced.
    ///
    unsafe fn drop_arc(&self) {
        drop(Arc::<SignalDataFull<T, F>>::from_raw(
            (self as *const Self).cast(),
        ))
    }

    fn touch(&self) -> &RwLock<T> {
        todo!()
    }

    fn pull(&self) -> &RwLock<T> {
        todo!()
    }
}
