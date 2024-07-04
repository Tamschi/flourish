use pollinate::runtime::SignalRuntimeRef;
use pollinate::runtime::Update;

mod raw_cached;
pub(crate) use raw_cached::RawCached;

mod raw_computed;
pub(crate) use raw_computed::RawComputed;

mod raw_computed_uncached;
pub(crate) use raw_computed_uncached::RawComputedUncached;

mod raw_computed_uncached_mut;
pub(crate) use raw_computed_uncached_mut::RawComputedUncachedMut;

mod raw_subject;
pub(crate) use raw_subject::RawSubject;

mod raw_folded;
pub(crate) use raw_folded::RawFolded;

mod raw_merged;
pub(crate) use raw_merged::RawMerged;

pub(crate) mod raw_subscription;

pub(crate) mod raw_effect;
pub(crate) use raw_effect::new_raw_unsubscribed_effect;

use crate::traits::SubscribableSource;

pub fn subject<T: Send, SR: SignalRuntimeRef>(initial_value: T, runtime: SR) -> RawSubject<T, SR> {
    RawSubject::with_runtime(initial_value, runtime)
}
#[macro_export]
#[doc(hidden)]
macro_rules! subject {
    ($source:expr) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
		super let subject = ::core::pin::pin!($crate::raw::subject(
            $source
        ));
        ::core::pin::Pin::into_ref(subject)
	}};
}
#[doc(hidden)]
pub use crate::subject;

pub fn cached<'a, T: 'a + Send + Clone, SR: 'a + SignalRuntimeRef>(
    source: impl 'a + SubscribableSource<SR, Value = T>,
) -> impl 'a + SubscribableSource<SR, Value = T> {
    RawCached::<T, _, SR>::new(source)
}
#[macro_export]
#[doc(hidden)]
macro_rules! cached {
    ($f:expr) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
		super let cached = ::core::pin::pin!($crate::raw::computed($f, $crate::GlobalSignalRuntime));
        ::core::pin::Pin::into_ref(cached)
	}};
}
#[doc(hidden)]
pub use crate::cached;
#[macro_export]
#[doc(hidden)]
macro_rules! cached_from_source {
    ($source:expr) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
		super let cached_from_source = ::core::pin::pin!($crate::raw::cached($source));
        ::core::pin::Pin::into_ref(cached_from_source)
	}};
}
#[doc(hidden)]
pub use crate::cached_from_source;

pub fn computed<'a, T: 'a + Send, F: 'a + Send + FnMut() -> T, SR: 'a + SignalRuntimeRef>(
    f: F,
    runtime: SR,
) -> impl 'a + SubscribableSource<SR, Value = T> {
    RawComputed::<T, _, SR>::new(f, runtime)
}
#[macro_export]
#[doc(hidden)]
macro_rules! computed {
    ($f:expr) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
		super let computed = ::core::pin::pin!($crate::raw::computed($f, $crate::GlobalSignalRuntime));
        ::core::pin::Pin::into_ref(computed)
	}};
}
#[doc(hidden)]
pub use crate::computed;
#[macro_export]
#[doc(hidden)]
macro_rules! computed_with_runtime {
    ($source:expr, $runtime:expr) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
		super let computed_with_runtime = ::core::pin::pin!($crate::raw::computed($source, $runtime));
        ::core::pin::Pin::into_ref(computed_with_runtime)
	}};
}
#[doc(hidden)]
pub use crate::computed_with_runtime;

pub fn computed_uncached<
    'a,
    T: 'a + Send,
    F: 'a + Send + Sync + Fn() -> T,
    SR: 'a + SignalRuntimeRef,
>(
    f: F,
    runtime: SR,
) -> impl 'a + SubscribableSource<SR, Value = T> {
    RawComputedUncached::<T, _, SR>::new(f, runtime)
}
#[macro_export]
#[doc(hidden)]
macro_rules! computed_uncached {
    ($f:expr) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
		super let computed_uncached = ::core::pin::pin!($crate::raw::computed_uncached($f, $crate::GlobalSignalRuntime));
        ::core::pin::Pin::into_ref(computed_uncached)
	}};
}
#[doc(hidden)]
pub use crate::computed_uncached;
#[macro_export]
#[doc(hidden)]
macro_rules! computed_uncached_with_runtime {
    ($source:expr, $runtime:expr) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
		super let computed_uncached_with_runtime = ::core::pin::pin!($crate::raw::computed_uncached($source, $runtime));
        ::core::pin::Pin::into_ref(computed_uncached_with_runtime)
	}};
}
#[doc(hidden)]
pub use crate::computed_uncached_with_runtime;

pub fn computed_uncached_mut<
    'a,
    T: 'a + Send,
    F: 'a + Send + FnMut() -> T,
    SR: 'a + SignalRuntimeRef,
>(
    f: F,
    runtime: SR,
) -> impl 'a + SubscribableSource<SR, Value = T> {
    RawComputedUncachedMut::<T, _, SR>::new(f, runtime)
}
#[macro_export]
#[doc(hidden)]
macro_rules! computed_uncached_mut {
    ($f:expr) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
		super let computed_uncached_mut = ::core::pin::pin!($crate::raw::computed_uncached_mut($f, $crate::GlobalSignalRuntime));
        ::core::pin::Pin::into_ref(computed_uncached_mut)
	}};
}
#[doc(hidden)]
pub use crate::computed_uncached_mut;
#[macro_export]
#[doc(hidden)]
macro_rules! computed_uncached_mut_with_runtime {
    ($source:expr, $runtime:expr) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
		super let computed_uncached_mut_with_runtime = ::core::pin::pin!($crate::raw::computed_uncached_mut($source, $runtime));
        ::core::pin::Pin::into_ref(computed_uncached_mut_with_runtime)
	}};
}
#[doc(hidden)]
pub use crate::computed_uncached_mut_with_runtime;

pub fn folded<'a, T: 'a + Send, SR: 'a + SignalRuntimeRef>(
    init: T,
    f: impl 'a + Send + FnMut(&mut T) -> Update,
    runtime: SR,
) -> impl 'a + SubscribableSource<SR, Value = T> {
    RawFolded::new(init, f, runtime)
}
#[macro_export]
#[doc(hidden)]
macro_rules! folded {
    ($select:expr, $init:expr, $fold:expr) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
		super let folded = ::core::pin::pin!($crate::raw::folded($crate::raw::computed_uncached_mut($select, $crate::GlobalSignalRuntime), $init, $fold));
        ::core::pin::Pin::into_ref(folded)
	}};
}
#[doc(hidden)]
pub use crate::folded;
#[macro_export]
#[doc(hidden)]
macro_rules! folded_from_source {
    ($source:expr, $init:expr, $f:expr) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
		super let folded = ::core::pin::pin!($crate::raw::folded($source, $init, $f));
        ::core::pin::Pin::into_ref(fold)
	}};
}
#[doc(hidden)]
pub use crate::folded_from_source;

pub fn merged<'a, T: 'a + Send, SR: 'a + SignalRuntimeRef>(
    select: impl 'a + Send + FnMut() -> T,
    merge: impl 'a + Send + FnMut(&mut T, T) -> Update,
    runtime: SR,
) -> impl 'a + SubscribableSource<SR, Value = T> {
    RawMerged::new(select, merge, runtime)
}
#[macro_export]
#[doc(hidden)]
macro_rules! merged {
    ($select:expr, $fold:expr) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
		super let merged = ::core::pin::pin!($crate::raw::merged($crate::raw::computed_uncached_mut($select, $crate::GlobalSignalRuntime), $fold));
        ::core::pin::Pin::into_ref(merged)
	}};
}
#[doc(hidden)]
pub use crate::merged;
#[macro_export]
#[doc(hidden)]
macro_rules! merged_with_runtime {
    ($select:expr, $merge:expr, $runtime:expr) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
		super let merged = ::core::pin::pin!($crate::raw::merged($select, $merge, $runtime));
        ::core::pin::Pin::into_ref(fold)
	}};
}
#[doc(hidden)]
pub use crate::merged_with_runtime;

#[macro_export]
#[doc(hidden)]
macro_rules! subscription {
    ($f:expr) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
        super let subscription = ::core::pin::pin!($crate::__::new_raw_unsubscribed_subscription(
            $crate::raw::computed($f, $crate::GlobalSignalRuntime)
        ));
        let subscription = ::core::pin::Pin::into_ref(subscription);
        $crate::__::pull_subscription(subscription);
        $crate::__::pin_into_pin_impl_source(subscription);
    }};
}
#[doc(hidden)]
pub use crate::subscription;
#[macro_export]
#[doc(hidden)]
macro_rules! subscription_with_runtime {
    ($f:expr, $runtime:expr) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
        super let subscription_with_runtime = ::core::pin::pin!($crate::__::new_raw_unsubscribed_subscription(
            $crate::raw::computed($f, $runtime)
        ));
        let subscription_with_runtime = ::core::pin::Pin::into_ref(subscription_with_runtime);
        $crate::__::pull_subscription(subscription_with_runtime);
        $crate::__::pin_into_pin_impl_source(subscription_with_runtime);
    }};
}
#[doc(hidden)]
pub use crate::subscription_with_runtime;
#[macro_export]
#[doc(hidden)]
macro_rules! subscription_from_source {
    ($source:expr) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
        super let subscription_from_source = ::core::pin::pin!($crate::__::new_raw_unsubscribed_subscription($source));
        let subscription_from_source = ::core::pin::Pin::into_ref(subscription_from_source);
        $crate::__::pull_subscription(subscription_from_source);
        $crate::__::pin_into_pin_impl_source(subscription_from_source);
    }};
}
#[doc(hidden)]
pub use crate::subscription_from_source;
#[macro_export]
#[doc(hidden)]
macro_rules! effect {
    ($f:expr, $drop:expr) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
        super let effect = ::core::pin::pin!($crate::__::new_raw_unsubscribed_effect(
            $f,
            $drop,
            $crate::GlobalSignalRuntime
        ));
        let effect = ::core::pin::Pin::into_ref(effect);
        $crate::__::pull_effect(effect);
    }};
}
#[doc(hidden)]
pub use crate::effect;
#[macro_export]
#[doc(hidden)]
macro_rules! effect_with_runtime {
    ($f:expr, $drop:expr, $runtime:expr) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
        super let effect_with_runtime = ::core::pin::pin!($crate::__::new_raw_unsubscribed_effect($f, $drop, $runtime));
        let effect_with_runtime = ::core::pin::Pin::into_ref(effect_with_runtime);
        $crate::__::pull_effect(effect_with_runtime);
    }};
}
#[doc(hidden)]
pub use crate::effect_with_runtime;

/// A helper to bind macros on the stack.
#[macro_export]
macro_rules! signals_helper {
	{let $name:ident = subject!($initial_value:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::raw::subject($initial_value, $crate::GlobalSignalRuntime));
		let $name = ::core::pin::Pin::into_ref($name);
	};
	{let $name:ident = subject_with_runtime!($initial_value:expr, $runtime:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::raw::subject($initial_value, $runtime));
		let $name = ::core::pin::Pin::into_ref($name);
	};
	{let $name:ident = cached!($source:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::raw::cached($source));
		let $name = $crate::SubscribableSource::ref_as_source(::core::pin::Pin::into_ref($name));
	};
	{let $name:ident = computed!($f:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::raw::computed($f, $crate::GlobalSignalRuntime));
		let $name = $crate::SubscribableSource::ref_as_source(::core::pin::Pin::into_ref($name));
	};
	{let $name:ident = computed_with_runtime!($f:expr, $runtime:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::raw::computed($f, $runtime));
		let $name = $crate::SubscribableSource::ref_as_source(::core::pin::Pin::into_ref($name));
	};
	{let $name:ident = computed_uncached!($f:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::raw::computed_uncached($f, $crate::GlobalSignalRuntime));
		let $name = $crate::SubscribableSource::ref_as_source(::core::pin::Pin::into_ref($name));
	};
	{let $name:ident = computed_uncached_with_runtime!($f:expr, $runtime:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::raw::computed_uncached($f, $runtime));
		let $name = $crate::SubscribableSource::ref_as_source(::core::pin::Pin::into_ref($name));
	};
	{let $name:ident = computed_uncached_mut!($f:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::raw::computed_uncached_mut($f, $crate::GlobalSignalRuntime));
		let $name = $crate::SubscribableSource::ref_as_source(::core::pin::Pin::into_ref($name));
	};
	{let $name:ident = computed_uncached_mut_with_runtime!($f:expr, $runtime:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::raw::computed_uncached_mut($f, $runtime));
		let $name = $crate::SubscribableSource::ref_as_source(::core::pin::Pin::into_ref($name));
	};
	{let $name:ident = folded!($init:expr, $f:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::raw::folded($init, $f, $crate::GlobalSignalRuntime));
		let $name = $crate::SubscribableSource::ref_as_source(::core::pin::Pin::into_ref($name));
	};
	{let $name:ident = folded_with_runtime!($init:expr, $f:expr, $runtime:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::raw::folded($init, $f, $runtime));
		let $name = $crate::SubscribableSource::ref_as_source(::core::pin::Pin::into_ref($name));
	};
	{let $name:ident = merged!($select:expr, $merge:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::raw::merged($select, $merge, $crate::GlobalSignalRuntime));
		let $name = $crate::SubscribableSource::ref_as_source(::core::pin::Pin::into_ref($name));
	};
	{let $name:ident = merged_with_runtime!($select:expr, $merge:expr, $runtime:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::raw::merged($select, $merge, $runtime));
		let $name = $crate::SubscribableSource::ref_as_source(::core::pin::Pin::into_ref($name));
	};
	{let $name:ident = subscription!($f:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::__::new_raw_unsubscribed_subscription($crate::raw::computed($f, $crate::GlobalSignalRuntime)));
		let $name = ::core::pin::Pin::into_ref($name);
		$crate::__::pull_subscription($name);
		let $name = $crate::__::pin_into_pin_impl_source($name);
	};
	{let $name:ident = subscription_with_runtime!($f:expr, $runtime:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::__::new_raw_unsubscribed_subscription($crate::raw::computed($f, $runtime)));
		let $name = ::core::pin::Pin::into_ref($name);
		$crate::__::pull_subscription($name);
		let $name = $crate::__::pin_into_pin_impl_source($name);
	};
	{let $name:ident = subscription_from_source!($source:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::__::new_raw_unsubscribed_subscription($source));
		let $name = ::core::pin::Pin::into_ref($name);
		$crate::__::pull_subscription($name);
		let $name = $crate::__::pin_into_pin_impl_source($name);
	};
	{let $name:ident = effect!($f:expr, $drop:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::__::new_raw_unsubscribed_effect($f, $drop, $crate::GlobalSignalRuntime));
		let $name = ::core::pin::Pin::into_ref($name);
		$name.pull();
	};
	{let $name:ident = effect_with_runtime!($f:expr, $drop:expr, $runtime:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::__::new_raw_unsubscribed_effect($f, $drop, $runtime));
		let $name = ::core::pin::Pin::into_ref($name);
		$name.pull();
	};
	// Error variant.
	{let $name:ident = $macro:ident!($($arg:expr),*$(,)?);} => {
		// Nicely squiggles the unrecognised identifier… in rust-analyzer.
		// The compiler still squiggles the entire macro, unfortunately.
		::core::compile_error!(::core::concat!(
			"Unrecognised macro name or wrong argument count (for) `", ::core::stringify!($macro), "`. The following macros are supported:\n",
			"subject[_with_runtime]!(1/2), cached!(1), computed[_uncached[_mut]][_with_runtime]!(1/2), ",
			"folded[_with_runtime]!(2/3), merged[_with_runtime]!(2/3), subscription[_with_runtime]!(1/2), ",
			"subscription_from_source!(1), effect[_with_runtime]!(2/3)"
		));
	};
	// Repeat.
	{$(let $name:ident = $macro:ident!($($arg:expr),*$(,)?);)*} => {$(
		$crate::raw::signals_helper! {
			let $name = $macro!($($arg),*);
		}
	)*};
}
pub use crate::signals_helper;
