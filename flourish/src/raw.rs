use pollinate::runtime::Update;
use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef};

mod raw_cached;
pub(crate) use raw_cached::{RawCached, RawCachedGuard};

mod raw_computed;
pub(crate) use raw_computed::{RawComputed, RawComputedGuard};

mod raw_subject;
pub(crate) use raw_subject::{RawSubject, RawSubjectGuard};

mod raw_folded;
pub(crate) use raw_folded::RawFolded;

mod raw_merged;
pub(crate) use raw_merged::RawMerged;

pub(crate) mod raw_subscription;
pub(crate) use raw_subscription::{new_raw_unsubscribed_subscription, pull_subscription};

use crate::Source;

pub fn subject<T: Send>(initial_value: T) -> RawSubject<T, GlobalSignalRuntime> {
    subject_with_runtime(initial_value, GlobalSignalRuntime)
}
#[macro_export]
macro_rules! subject {
    ($source:expr) => {{
		super let subject = ::core::pin::pin!($crate::raw::subject($source))
        ::core::pin::Pin::into_ref(subject)
	}};
}
pub use crate::subject;

pub fn subject_with_runtime<T: Send, SR: SignalRuntimeRef>(
    initial_value: T,
    runtime: SR,
) -> RawSubject<T, SR> {
    RawSubject::with_runtime(initial_value, runtime)
}
#[macro_export]
macro_rules! subject_with_runtime {
    ($source:expr) => {{
		super let subject_with_runtime = ::core::pin::pin!($crate::raw::subject_with_runtime(
            $source
        ));
        ::core::pin::Pin::into_ref(subject_with_runtime)
	}};
}
pub use crate::subject_with_runtime;

pub fn cached<'a, T: 'a + Send + Clone, SR: 'a + SignalRuntimeRef>(
    source: impl 'a + Source<SR, Value = T>,
) -> impl 'a + Source<SR, Value = T> {
    RawCached::<T, _, SR>::new(source)
}
#[macro_export]
macro_rules! cached {
    ($f:expr) => {{
		super let cached = ::core::pin::pin!($crate::raw::cached(($f, $crate::GlobalSignalRuntime)));
        ::core::pin::Pin::into_ref(cached)
	}};
}
pub use crate::cached;
#[macro_export]
macro_rules! cached_from_source {
    ($source:expr) => {{
		super let cached_from_source = ::core::pin::pin!($crate::raw::cached($source));
        ::core::pin::Pin::into_ref(cached_from_source)
	}};
}
pub use crate::cached_from_source;

pub fn computed<
    'a,
    T: 'a + Send + Clone,
    F: 'a + Send + FnMut() -> T,
    SR: 'a + SignalRuntimeRef,
>(
    f: F,
    runtime: SR,
) -> impl 'a + Source<SR, Value = T> {
    RawComputed::<T, _, SR>::new(f, runtime)
}
#[macro_export]
macro_rules! computed {
    ($f:expr) => {{
		super let computed = ::core::pin::pin!($crate::raw::computed($f, $crate::GlobalSignalRuntime));
        ::core::pin::Pin::into_ref(computed)
	}};
}
pub use crate::computed;
#[macro_export]
macro_rules! computed_with_runtime {
    ($source:expr, $runtime:expr) => {{
		super let computed_with_runtime = ::core::pin::pin!($crate::raw::computed($source, $runtime));
        ::core::pin::Pin::into_ref(computed_with_runtime)
	}};
}
pub use crate::computed_with_runtime;

pub fn folded<'a, B: 'a + Send, T: 'a + Send + Clone, SR: 'a + SignalRuntimeRef>(
    source: impl 'a + Source<SR, Value = T>,
    init: B,
    f: impl 'a + Send + FnMut(&mut B, T) -> Update,
) -> impl 'a + Source<SR, Value = B> {
    RawFolded::new(source, init, f)
}
#[macro_export]
macro_rules! folded {
    ($selector:expr, $init:expr, $fold:expr) => {{
		super let folded = ::core::pin::pin!($crate::raw::folded(($selector, $crate::GlobalSignalRuntime), $init, $fold));
        ::core::pin::Pin::into_ref(folded)
	}};
}
pub use crate::folded;
#[macro_export]
macro_rules! folded_from_source {
    ($source:expr, $init:expr, $f:expr) => {{
		super let folded = ::core::pin::pin!($crate::raw::folded($source, $init, $f));
        ::core::pin::Pin::into_ref(fold)
	}};
}
pub use crate::folded_from_source;

pub fn merged<'a, T: 'a + Send + Clone, SR: 'a + SignalRuntimeRef>(
    source: impl 'a + Source<SR, Value = T>,
    f: impl 'a + Send + FnMut(&mut T, T) -> Update,
) -> impl 'a + Source<SR, Value = T> {
    RawMerged::new(source, f)
}
#[macro_export]
macro_rules! merged {
    ($selector:expr, $fold:expr) => {{
		super let merged = ::core::pin::pin!($crate::raw::merged(($selector, $crate::GlobalSignalRuntime), $fold));
        ::core::pin::Pin::into_ref(merged)
	}};
}
pub use crate::merged;
#[macro_export]
macro_rules! merged_from_source {
    ($source:expr, $f:expr) => {{
		super let merged = ::core::pin::pin!($crate::raw::merged($source, $f));
        ::core::pin::Pin::into_ref(fold)
	}};
}
pub use crate::merged_from_source;

pub fn uncached<'a, T: 'a + Send, SR: 'a + SignalRuntimeRef>(
    source: impl 'a + Source<SR, Value = T>,
) -> impl 'a + Source<SR, Value = T> {
    let clone_runtime_ref = source.clone_runtime_ref();
    {
        let _ = clone_runtime_ref;
        source
    }
}
#[macro_export]
macro_rules! uncached {
    ($f:expr) => {{
		super let uncached = ::core::pin::pin!($crate::raw::uncached(($f, $crate::GlobalSignalRuntime)));
        ::core::pin::Pin::into_ref(uncached)
    }};
}
pub use crate::uncached;
#[macro_export]
macro_rules! uncached_from_source {
    ($source:expr) => {{
		super let uncached = ::core::pin::pin!($crate::raw::uncached($source));
        ::core::pin::Pin::into_ref(uncached)
    }};
}
pub use crate::uncached_from_source;

#[macro_export]
macro_rules! signals_helper {
	{let $name:ident = subject!($initial_value:expr);} => {
		let $name = ::core::pin::pin!($crate::raw::subject($initial_value));
		let $name = ::core::pin::Pin::into_ref($name);
	};
	{let $name:ident = subject_with_runtime!($initial_value:expr, $runtime:expr);} => {
		let $name = ::core::pin::pin!($crate::raw::subject_with_runtime($initial_value, $runtime));
		let $name = ::core::pin::Pin::into_ref($name);
	};
	{let $name:ident = cached!($f:expr);} => {
		let $name = ::core::pin::pin!($crate::raw::cached(($f, $crate::GlobalSignalRuntime)));
		let $name = ::core::pin::Pin::into_ref($name);
	};
	{let $name:ident = cached_from_source!($source:expr);} => {
		let $name = ::core::pin::pin!($crate::raw::cached($source));
		let $name = ::core::pin::Pin::into_ref($name);
	};
	{let $name:ident = computed!($f:expr);} => {
		let $name = ::core::pin::pin!($crate::raw::computed($f, $crate::GlobalSignalRuntime));
		let $name = ::core::pin::Pin::into_ref($name);
	};
	{let $name:ident = computed_with_runtime!($source:expr, $runtime:expr);} => {
		let $name = ::core::pin::pin!($crate::raw::computed($source, $runtime));
		let $name = ::core::pin::Pin::into_ref($name);
	};
	{let $name:ident = merged!($selector:expr, $fold:expr);} => {
		let $name = ::core::pin::pin!($crate::raw::merged(($selector, $crate::GlobalSignalRuntime), $fold));
		let $name = ::core::pin::Pin::into_ref($name);
	};
	{let $name:ident = merged_from_source!($source:expr, $f:expr);} => {
		let $name = ::core::pin::pin!($crate::raw::merged($source, $f));
		let $name = ::core::pin::Pin::into_ref($name);
	};
	{let $name:ident = uncached!($f:expr);} => {
		let $name = ::core::pin::pin!($crate::raw::uncached(($f, $crate::GlobalSignalRuntime)));
		let $name = ::core::pin::Pin::into_ref($name);
	};
	{let $name:ident = uncached_from_source!($source:expr);} => {
		let $name = ::core::pin::pin!($crate::raw::uncached($source));
		let $name = ::core::pin::Pin::into_ref($name);
	};
	{let $name:ident = subscription!($f:expr);} => {
		let $name = ::core::pin::pin!($crate::__::new_raw_unsubscribed_subscription(($f, $crate::GlobalSignalRuntime)));
		let $name = ::core::pin::Pin::into_ref($name);
		$crate::__::pull_subscription($name);
		let $name = $crate::__::pin_into_pin_impl_source($name);
	};
	{let $name:ident = subscription_from_source!($source:expr);} => {
		let $name = ::core::pin::pin!($crate::__::new_raw_unsubscribed_subscription($source));
		let $name = ::core::pin::Pin::into_ref($name);
		$crate::__::pull_subscription($name);
		let $name = $crate::__::pin_into_pin_impl_source($name);
	};
	{$(let $name:ident = $macro:ident!($source:expr$(, $runtime:expr)?);)*} => {$(
		$crate::raw::signals_helper! {
			let $name = $macro!($source$(, $runtime)?);
		}
	)*};
}
pub use crate::signals_helper;
