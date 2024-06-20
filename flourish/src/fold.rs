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
use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef, Update};
use sptr::{from_exposed_addr, Strict};

use crate::{raw::RawFold, AsSource, Source};

#[derive(Debug)]
pub struct Fold<T: Send + ?Sized, SR: SignalRuntimeRef = GlobalSignalRuntime>(
    /// In theory it's possible to store an invalid `*const T` here,
    /// in order to store pointer metadata, which would allow working with unsized type, maybe.
    NonNull<FoldHeader<T, SR>>,
);

unsafe impl<T: Send + ?Sized, SR: SignalRuntimeRef + Send> Send for Fold<T, SR> {}
unsafe impl<T: Send + ?Sized, SR: SignalRuntimeRef + Sync> Sync for Fold<T, SR> {}

impl<T: Send + ?Sized, SR: SignalRuntimeRef + Clone> Clone for Fold<T, SR> {
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

impl<T: Send + ?Sized, SR: SignalRuntimeRef> Drop for Fold<T, SR> {
    fn drop(&mut self) {
        // I think this is synchronised by dropping the Arc.
        let address = unsafe { self.0.as_ptr().read() }.0;
        unsafe { address.as_ref().drop_arc() };
    }
}

pub struct FoldGuard<'a, T: ?Sized>(RwLockReadGuard<'a, T>);

impl<'a, T: ?Sized> Deref for FoldGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl<'a, T: ?Sized> Borrow<T> for FoldGuard<'a, T> {
    fn borrow(&self) -> &T {
        self.0.borrow()
    }
}

/// See [rust-lang#98931](https://github.com/rust-lang/rust/issues/98931).
impl<T: Send + ?Sized> Fold<T> {
    pub fn new<S: Send + FnMut() -> T, M: Send + FnMut(&mut T, T) -> Update>(
        select: S,
        merge: M,
    ) -> Self
    where
        T: Sized,
    {
        Self::with_runtime(select, merge, GlobalSignalRuntime)
    }
}

impl<T: Send + ?Sized, SR: SignalRuntimeRef> Fold<T, SR> {
    pub fn with_runtime<S: Send + FnMut() -> T, M: Send + FnMut(&mut T, T) -> Update>(
        select: S,
        merge: M,
        runtime: SR,
    ) -> Self
    where
        T: Sized,
    {
        let arc = Arc::new(FoldDataFull {
            anchor: FoldDataAnchor(PhantomData),
            header: Cell::new(MaybeUninit::uninit()),
            fold: RawFold::with_runtime(select, merge, runtime),
        });
        unsafe {
            arc.header
                .set(MaybeUninit::new(FoldHeader(NonNull::new_unchecked(
                    &arc.anchor as &dyn FoldDataAddress<T, SR> as *const _ as *mut _,
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

    pub fn read<'a>(&'a self) -> FoldGuard<'a, T>
    where
        T: Sync,
    {
        FoldGuard(self.touch().read().unwrap())
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

    pub fn as_source(&self) -> Pin<&(dyn Source<Value = T> + Sync)>
    where
        SR: Sync,
    {
        let address = unsafe { self.0.as_ptr().read() }.0;
        unsafe { address.as_ref().as_source() }
    }
}

/// FIXME: Once pointer-metadata is available, shrink this.
#[derive(Debug, Clone, Copy)]
struct FoldHeader<T: Send + ?Sized, SR: SignalRuntimeRef>(NonNull<dyn FoldDataAddress<T, SR>>);

trait FoldDataAddress<T: Send + ?Sized, SR: SignalRuntimeRef> {
    unsafe fn drop_arc(&self);
    fn touch(&self) -> &RwLock<T>;
    fn pull(&self) -> &RwLock<T>;
    fn as_source(&self) -> Pin<&(dyn Source<Value = T> + Sync)>
    where
        SR: Sync;
}

#[pin_project]
#[repr(C)]
struct FoldDataFull<
    T: Send,
    S: Send + FnMut() -> T,
    M: Send + FnMut(&mut T, T) -> Update,
    SR: SignalRuntimeRef,
> {
    anchor: FoldDataAnchor<T, S, M, SR>,
    header: Cell<MaybeUninit<FoldHeader<T, SR>>>,
    #[pin]
    fold: RawFold<T, S, M, SR>,
}

/// MUST BE A ZST
struct FoldDataAnchor<
    T: Send,
    S: Send + FnMut() -> T,
    M: Send + FnMut(&mut T, T) -> Update,
    SR: SignalRuntimeRef,
>(PhantomData<*const FoldDataFull<T, S, M, SR>>);

impl<
        T: Send,
        S: Send + FnMut() -> T,
        M: Send + FnMut(&mut T, T) -> Update,
        SR: SignalRuntimeRef,
    > FoldDataAddress<T, SR> for FoldDataAnchor<T, S, M, SR>
{
    /// # Safety
    ///
    /// `Self` is a ZST, so it's not actually dereferenced.
    ///
    unsafe fn drop_arc(&self) {
        drop(Arc::<FoldDataFull<T, S, M, SR>>::from_raw(
            from_exposed_addr(Strict::addr(self as *const Self)),
        ))
    }

    fn touch(&self) -> &RwLock<T> {
        unsafe {
            Pin::new_unchecked(&*from_exposed_addr::<FoldDataFull<T, S, M, SR>>(
                Strict::addr(self as *const Self),
            ))
        }
        .project_ref()
        .fold
        .touch()
    }

    fn pull(&self) -> &RwLock<T> {
        unsafe {
            Pin::new_unchecked(&*from_exposed_addr::<FoldDataFull<T, S, M, SR>>(
                Strict::addr(self as *const Self),
            ))
        }
        .project_ref()
        .fold
        .pull()
    }

    fn as_source(&self) -> Pin<&(dyn Source<Value = T> + Sync)>
    where
        SR: Sync,
    {
        unsafe {
            Pin::new_unchecked(&*from_exposed_addr::<FoldDataFull<T, S, M, SR>>(
                Strict::addr(self as *const Self),
            ))
        }
        .project_ref()
        .fold
    }
}

impl<'a, T: 'a + Send + ?Sized, SR: 'a + Sync + SignalRuntimeRef> AsSource<'a> for Fold<T, SR> {
    type Source = dyn 'a + Source<Value = T> + Sync;

    fn as_source(self: Pin<&Self>) -> Pin<&Self::Source> {
        let address = unsafe { self.0.as_ptr().read() }.0;
        unsafe { address.as_ref().as_source() }
    }
}
