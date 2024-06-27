mod raw_computed;
use pollinate::runtime::Update;
use pollinate::runtime::{GlobalSignalRuntime, SignalRuntimeRef};
pub use raw_computed::{RawComputed, RawComputedGuard};

mod raw_subject;
pub(crate) use raw_subject::{RawSubject, RawSubjectGuard};

mod raw_fold;
pub(crate) use raw_fold::RawFold;

pub(crate) mod raw_subscription;
pub(crate) use raw_subscription::{
    new_raw_unsubscribed_subscription_with_runtime, pull_subscription,
};

use crate::Source;

pub fn subject<T: Send>(initial_value: T) -> RawSubject<T, GlobalSignalRuntime> {
    subject_sr(initial_value, GlobalSignalRuntime)
}
#[macro_export]
macro_rules! subject {
    ($source:expr) => {{
		super let subject = ::core::pin::pin!($crate::raw::subject($source))
        ::core::pin::Pin::into_ref(subject)
	}};
}
pub use crate::subject;

pub fn subject_sr<T: Send, SR: SignalRuntimeRef>(
    initial_value: T,
    runtime: SR,
) -> RawSubject<T, SR> {
    RawSubject::with_runtime(initial_value, runtime)
}
#[macro_export]
macro_rules! subject_sr {
    ($source:expr, $runtime:expr) => {{
		super let subject_sr = ::core::pin::pin!($crate::raw::subject_sr(
            $source, $runtime
        ));
        ::core::pin::Pin::into_ref(subject_sr)
	}};
}
pub use crate::subject_sr;

pub fn computed<'a, T: 'a + Send + Clone, SR: 'a + SignalRuntimeRef>(
    source: impl 'a + Source<SR, Value = T>,
) -> impl 'a + Source<SR, Value = T> {
    let runtime = source.clone_runtime_ref();
    RawComputed::<T, _, SR>::with_runtime(source, runtime)
}
#[macro_export]
macro_rules! computed {
    ($source:expr) => {{
		super let computed = ::core::pin::pin!($crate::raw::computed($source));
        ::core::pin::Pin::into_ref(computed)
	}};
}
pub use crate::computed;

pub fn fold<'a, T: 'a + Send>(
    select: impl 'a + Send + FnMut() -> T,
    merge: impl 'a + Send + FnMut(&mut T, T) -> Update,
) -> impl 'a + Source<GlobalSignalRuntime, Value = T> {
    fold_sr(select, merge, GlobalSignalRuntime)
}
#[macro_export]
macro_rules! fold {
    ($source:expr) => {{
		super let fold = ::core::pin::pin!($crate::raw::fold($source));
        ::core::pin::Pin::into_ref(fold)
	}};
}
pub use crate::fold;

pub fn fold_sr<'a, T: 'a + Send, SR: 'a + SignalRuntimeRef>(
    select: impl 'a + Send + FnMut() -> T,
    merge: impl 'a + Send + FnMut(&mut T, T) -> Update,
    runtime: SR,
) -> impl 'a + Source<SR, Value = T> {
    RawFold::with_runtime(select, merge, runtime)
}
#[macro_export]
macro_rules! fold_sr {
    ($source:expr, $runtime:expr) => {{
		super let fold_sr = ::core::pin::pin!($crate::raw::fold_sr(
            $source, $runtime
        ));
        ::core::pin::Pin::into_ref(fold_sr)
	}};
}
pub use crate::fold_sr;

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
    ($source:expr) => {{
		super let uncached = ::core::pin::pin!($crate::raw::uncached($source));
        ::core::pin::Pin::into_ref(uncached)
    }};
}
pub use crate::uncached;

#[macro_export]
macro_rules! signals_helper {
	{let $name:ident = subject!($initial_value:expr);} => {
		let $name = ::core::pin::pin!($crate::raw::subject($initial_value));
		let $name = ::core::pin::Pin::into_ref($name);
	};
	{let $name:ident = subject_sr!($initial_value:expr, $runtime:expr);} => {
		let $name = ::core::pin::pin!($crate::raw::subject_sr($initial_value, $runtime));
		let $name = ::core::pin::Pin::into_ref($name);
	};
	{let $name:ident = computed!($source:expr);} => {
		let $name = ::core::pin::pin!($crate::raw::computed($source));
		let $name = ::core::pin::Pin::into_ref($name);
	};
	{let $name:ident = computed_sr!($source:expr, $runtime:expr);} => {
		let $name = ::core::pin::pin!($crate::raw::computed_sr($source, $runtime));
		let $name = ::core::pin::Pin::into_ref($name);
	};
	{let $name:ident = uncached!($source:expr);} => {
		let $name = ::core::pin::pin!($crate::raw::uncached($source));
		let $name = ::core::pin::Pin::into_ref($name);
	};
	{let $name:ident = uncached_sr!($source:expr, $runtime:expr);} => {
		let $name = ::core::pin::pin!($crate::raw::uncached_sr($source, $runtime));
		let $name = ::core::pin::Pin::into_ref($name);
	};
	{let $name:ident = subscription!($source:expr);} => {
		let $name = ::core::pin::pin!($crate::__::new_raw_unsubscribed_subscription_with_runtime($source, $crate::GlobalSignalRuntime));
		let $name = ::core::pin::Pin::into_ref($name);
		$crate::__::pull_subscription($name);
		let $name = $crate::__::pin_into_pin_impl_source($name);
	};
	{let $name:ident = subscription_sr!($source:expr, $runtime:expr);} => {
		let $name = ::core::pin::pin!($crate::__::new_raw_unsubscribed_subscription_with_runtime($source, $runtime));
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
