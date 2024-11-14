//! This module contains unmanaged signals that can be pinned directly on the stack.
//!
//! In most application code, you should use [`Signal`](`crate::Signal`) instead,
//! which abstracts memory management and keeping track of subscriptions.
//!
//! Still, these building blocks are sometimes useful for composition and abstraction.
//!
//! To instantiate-and-pin unmanaged signals directly, it's currently most convenient to
//! use the [`signals_helper`] macro.

use isoprenoid::runtime::{CallbackTableTypes, Propagation, SignalsRuntimeRef};

pub use crate::traits::{UnmanagedSignal, UnmanagedSignalCell};

mod cached;
pub(crate) use cached::Cached;

mod computed;
pub(crate) use computed::Computed;

mod computed_uncached;
pub(crate) use computed_uncached::ComputedUncached;

mod computed_uncached_mut;
pub(crate) use computed_uncached_mut::ComputedUncachedMut;

mod inert_cell;
pub(crate) use inert_cell::InertCell;

mod reactive_cell;
pub(crate) use reactive_cell::ReactiveCell;

mod reactive_cell_mut;
pub(crate) use reactive_cell_mut::ReactiveCellMut;

mod folded;
pub(crate) use folded::Folded;

//TODO?: folded_emplaced
//TODO?: folded_with

mod reduced;
pub(crate) use reduced::Reduced;

pub(crate) mod raw_subscription;

pub(crate) mod raw_effect;
pub(crate) use raw_effect::new_raw_unsubscribed_effect;

//TODO: Can the individual macro placeholders in this module still communicate their eventual return type?

/// Unmanaged version of [`Signal::cell_with_runtime`](`crate::Signal::cell_with_runtime`).
pub fn inert_cell<T: Send, SR: SignalsRuntimeRef>(
	initial_value: T,
	runtime: SR,
) -> impl UnmanagedSignalCell<T, SR> {
	InertCell::with_runtime(initial_value, runtime)
}
#[macro_export]
#[doc(hidden)]
macro_rules! inert_cell {
    ($source:expr$(,)?) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
	}};
}
#[doc(hidden)]
pub use crate::inert_cell;
#[macro_export]
#[doc(hidden)]
macro_rules! inert_cell_with_runtime {
    ($source:expr, $runtime:expr$(,)?) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
	}};
}
#[doc(hidden)]
pub use crate::inert_cell_with_runtime;

/// Unmanaged version of [`Signal::cell_reactive_with_runtime`](`crate::Signal::cell_reactive_with_runtime`).
pub fn reactive_cell<
	T: Send,
	H: Send
		+ FnMut(&T, <SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus) -> Propagation,
	SR: SignalsRuntimeRef,
>(
	initial_value: T,
	on_subscribed_change_fn_pin: H,
	runtime: SR,
) -> impl UnmanagedSignalCell<T, SR> {
	ReactiveCell::with_runtime(initial_value, on_subscribed_change_fn_pin, runtime)
}
#[macro_export]
#[doc(hidden)]
macro_rules! reactive_cell {
    ($source:expr, $on_subscribed_change_fn_pin:expr$(,)?) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
	}};
}
#[doc(hidden)]
pub use crate::reactive_cell;
#[macro_export]
#[doc(hidden)]
macro_rules! reactive_cell_with_runtime {
    ($source:expr, $runtime:expr$(,)?) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
	}};
}
#[doc(hidden)]
pub use crate::reactive_cell_with_runtime;

/// Unmanaged version of [`Signal::cell_reactive_mut_with_runtime`](`crate::Signal::cell_reactive_mut_with_runtime`).
pub fn reactive_cell_mut<
	T: Send,
	H: Send
		+ FnMut(
			&mut T,
			<SR::CallbackTableTypes as CallbackTableTypes>::SubscribedStatus,
		) -> Propagation,
	SR: SignalsRuntimeRef,
>(
	initial_value: T,
	on_subscribed_change_fn_pin: H,
	runtime: SR,
) -> impl UnmanagedSignalCell<T, SR> {
	ReactiveCellMut::with_runtime(initial_value, on_subscribed_change_fn_pin, runtime)
}
#[macro_export]
#[doc(hidden)]
macro_rules! reactive_cell_mut {
    ($source:expr, $on_subscribed_change_fn_pin:expr$(,)?) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
	}};
}
#[doc(hidden)]
pub use crate::reactive_cell_mut;
#[macro_export]
#[doc(hidden)]
macro_rules! reactive_cell_mut_with_runtime {
    ($source:expr, $runtime:expr$(,)?) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
	}};
}
#[doc(hidden)]
pub use crate::reactive_cell_mut_with_runtime;

/// Wraps another [`UnmanagedSignal`] to add a result cache.
pub fn cached<'a, T: 'a + Send + Clone, SR: 'a + SignalsRuntimeRef>(
	source: impl 'a + UnmanagedSignal<T, SR>,
) -> impl 'a + UnmanagedSignal<T, SR> {
	Cached::<T, _, SR>::new(source)
}
#[macro_export]
#[doc(hidden)]
macro_rules! cached {
    ($fn_pin:expr$(,)?) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
	}};
}
#[doc(hidden)]
pub use crate::cached;
#[macro_export]
#[doc(hidden)]
macro_rules! cached_from_source {
    ($source:expr$(,)?) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
	}};
}
#[doc(hidden)]
pub use crate::cached_from_source;

/// Unmanaged version of [`Signal::computed_with_runtime`](`crate::Signal::computed_with_runtime`).
pub fn computed<'a, T: 'a + Send, F: 'a + Send + FnMut() -> T, SR: 'a + SignalsRuntimeRef>(
	fn_pin: F,
	runtime: SR,
) -> impl 'a + UnmanagedSignal<T, SR> {
	Computed::<T, _, SR>::new(fn_pin, runtime)
}
#[macro_export]
#[doc(hidden)]
macro_rules! computed {
    ($fn_pin:expr$(,)?) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
	}};
}
#[doc(hidden)]
pub use crate::computed;
#[macro_export]
#[doc(hidden)]
macro_rules! computed_with_runtime {
    ($source:expr, $runtime:expr$(,)?) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
	}};
}
#[doc(hidden)]
pub use crate::computed_with_runtime;

/// Unmanaged version of [`Signal::debounced_with_runtime`](`crate::Signal::debounced_with_runtime`).
pub fn debounced<
	'a,
	T: 'a + Send + PartialEq,
	F: 'a + Send + FnMut() -> T,
	SR: 'a + SignalsRuntimeRef,
>(
	fn_pin: F,
	runtime: SR,
) -> impl 'a + UnmanagedSignal<T, SR> {
	Reduced::<T, _, _, SR>::new(
		fn_pin,
		|value, new_value| {
			if *value != new_value {
				*value = new_value;
				Propagation::Propagate
			} else {
				Propagation::Halt
			}
		},
		runtime,
	)
}
#[macro_export]
#[doc(hidden)]
macro_rules! debounced {
    ($fn_pin:expr$(,)?) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
	}};
}
#[doc(hidden)]
pub use crate::debounced;
#[macro_export]
#[doc(hidden)]
macro_rules! debounced_with_runtime {
    ($source:expr, $runtime:expr$(,)?) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
	}};
}
#[doc(hidden)]
pub use crate::debounced_with_runtime;

/// Unmanaged version of [`Signal::computed_uncached_with_runtime`](`crate::Signal::computed_uncached_with_runtime`).
pub fn computed_uncached<
	'a,
	T: 'a + Send,
	F: 'a + Send + Sync + Fn() -> T,
	SR: 'a + SignalsRuntimeRef,
>(
	fn_pin: F,
	runtime: SR,
) -> impl 'a + UnmanagedSignal<T, SR> {
	ComputedUncached::<T, _, SR>::new(fn_pin, runtime)
}
#[macro_export]
#[doc(hidden)]
macro_rules! computed_uncached {
    ($fn_pin:expr$(,)?) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
	}};
}
#[doc(hidden)]
pub use crate::computed_uncached;
#[macro_export]
#[doc(hidden)]
macro_rules! computed_uncached_with_runtime {
    ($source:expr, $runtime:expr$(,)?) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
	}};
}
#[doc(hidden)]
pub use crate::computed_uncached_with_runtime;

/// Unmanaged version of [`Signal::computed_uncached_mut_with_runtime`](`crate::Signal::computed_uncached_mut_with_runtime`).
pub fn computed_uncached_mut<
	'a,
	T: 'a + Send,
	F: 'a + Send + FnMut() -> T,
	SR: 'a + SignalsRuntimeRef,
>(
	fn_pin: F,
	runtime: SR,
) -> impl 'a + UnmanagedSignal<T, SR> {
	ComputedUncachedMut::<T, _, SR>::new(fn_pin, runtime)
}
#[macro_export]
#[doc(hidden)]
macro_rules! computed_uncached_mut {
    ($fn_pin:expr$(,)?) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
	}};
}
#[doc(hidden)]
pub use crate::computed_uncached_mut;
#[macro_export]
#[doc(hidden)]
macro_rules! computed_uncached_mut_with_runtime {
    ($source:expr, $runtime:expr$(,)?) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
	}};
}
#[doc(hidden)]
pub use crate::computed_uncached_mut_with_runtime;

/// Unmanaged version of [`Signal::folded_with_runtime`](`crate::Signal::folded_with_runtime`).
pub fn folded<'a, T: 'a + Send, SR: 'a + SignalsRuntimeRef>(
	init: T,
	fold_fn_pin: impl 'a + Send + FnMut(&mut T) -> Propagation,
	runtime: SR,
) -> impl 'a + UnmanagedSignal<T, SR> {
	Folded::new(init, fold_fn_pin, runtime)
}
#[macro_export]
#[doc(hidden)]
macro_rules! folded {
    ($init:expr, $fold_fn_pin:expr$(,)?) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
	}};
}
#[doc(hidden)]
pub use crate::folded;

/// Unmanaged version of [`Signal::reduced_with_runtime`](`crate::Signal::reduced_with_runtime`).
pub fn reduced<'a, T: 'a + Send, SR: 'a + SignalsRuntimeRef>(
	select_fn_pin: impl 'a + Send + FnMut() -> T,
	reduce_fn_pin: impl 'a + Send + FnMut(&mut T, T) -> Propagation,
	runtime: SR,
) -> impl 'a + UnmanagedSignal<T, SR> {
	Reduced::new(select_fn_pin, reduce_fn_pin, runtime)
}
#[macro_export]
#[doc(hidden)]
macro_rules! reduced {
    ($select_fn_pin:expr, $reduce_fn_pin:expr$(,)?) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
	}};
}
#[doc(hidden)]
pub use crate::reduced;
#[macro_export]
#[doc(hidden)]
macro_rules! reduced_with_runtime {
    ($select_fn_pin:expr, $reduce_fn_pin:expr, $runtime:expr$(,)?) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
	}};
}
#[doc(hidden)]
pub use crate::reduced_with_runtime;

#[macro_export]
#[doc(hidden)]
macro_rules! subscription {
    ($fn_pin:expr$(,)?) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
    }};
}
#[doc(hidden)]
pub use crate::subscription;
#[macro_export]
#[doc(hidden)]
macro_rules! subscription_with_runtime {
    ($fn_pin:expr, $runtime:expr$(,)?) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
    }};
}
#[doc(hidden)]
pub use crate::subscription_with_runtime;
#[macro_export]
#[doc(hidden)]
macro_rules! subscription_from_source {
    ($source:expr$(,)?) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
    }};
}
#[doc(hidden)]
pub use crate::subscription_from_source;
#[macro_export]
#[doc(hidden)]
macro_rules! effect {
    ($fn_pin:expr, $drop:expr$(,)?) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
    }};
}
#[doc(hidden)]
pub use crate::effect;
#[macro_export]
#[doc(hidden)]
macro_rules! effect_with_runtime {
    ($fn_pin:expr, $drop:expr, $runtime:expr$(,)?) => {{
		::core::compile_error!("Using this macro directly would require `super let`. For now, please wrap the binding(s) in `signals_helper! { … }`.");
    }};
}
#[doc(hidden)]
pub use crate::effect_with_runtime;

/// A helper to pin [`unmanaged`](`self`) signals on the stack.  
/// Canonically [`unmanaged::signals_helper`](`signals_helper`).
///
/// See [`unmanaged`#functions](`self`#functions) for help on individual patterns.
///
/// The last two branches improve error messages and enable repetitions, respectively.
#[macro_export]
macro_rules! signals_helper {
	{let $name:ident = inert_cell!($initial_value:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::unmanaged::inert_cell($initial_value, $crate::GlobalSignalsRuntime));
		let $name = ::core::pin::Pin::into_ref($name);
	};
	{let $name:ident = inert_cell_with_runtime!($initial_value:expr, $runtime:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::unmanaged::inert_cell($initial_value, $runtime));
		let $name = ::core::pin::Pin::into_ref($name);
	};
	{let $name:ident = reactive_cell!($initial_value:expr, $on_subscribed_change_fn_pin:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::unmanaged::reactive_cell($initial_value, $on_subscribed_change_fn_pin, $crate::GlobalSignalsRuntime));
		let $name = ::core::pin::Pin::into_ref($name);
	};
	{let $name:ident = reactive_cell_with_runtime!($initial_value:expr, $on_subscribed_change_fn_pin:expr, $runtime:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::unmanaged::reactive_cell($initial_value, $on_subscribed_change_fn_pin, $runtime));
		let $name = ::core::pin::Pin::into_ref($name);
	};
	{let $name:ident = reactive_cell_mut!($initial_value:expr, $on_subscribed_change_fn_pin:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::unmanaged::reactive_cell_mut($initial_value, $on_subscribed_change_fn_pin, $crate::GlobalSignalsRuntime));
		let $name = ::core::pin::Pin::into_ref($name);
	};
	{let $name:ident = reactive_cell_mut_with_runtime!($initial_value:expr, $on_subscribed_change_fn_pin:expr, $runtime:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::unmanaged::reactive_cell_mut($initial_value, $on_subscribed_change_fn_pin, $runtime));
		let $name = ::core::pin::Pin::into_ref($name);
	};
	{let $name:ident = cached!($source:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::unmanaged::cached($source));
		let $name = ::core::pin::Pin::into_ref($name) as ::core::pin::Pin<&dyn $crate::unmanaged::UnmanagedSignal<_, _>>;
	};
	{let $name:ident = computed!($fn_pin:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::unmanaged::computed($fn_pin, $crate::GlobalSignalsRuntime));
		let $name = ::core::pin::Pin::into_ref($name) as ::core::pin::Pin<&dyn $crate::unmanaged::UnmanagedSignal<_, _>>;
	};
	{let $name:ident = computed_with_runtime!($fn_pin:expr, $runtime:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::unmanaged::computed($fn_pin, $runtime));
		let $name = ::core::pin::Pin::into_ref($name) as ::core::pin::Pin<&dyn $crate::unmanaged::UnmanagedSignal<_, _>>;
	};
	{let $name:ident = debounced!($fn_pin:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::unmanaged::debounced($fn_pin, $crate::GlobalSignalsRuntime));
		let $name = ::core::pin::Pin::into_ref($name) as ::core::pin::Pin<&dyn $crate::unmanaged::UnmanagedSignal<_, _>>;
	};
	{let $name:ident = debounced_with_runtime!($fn_pin:expr, $runtime:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::unmanaged::debounced($fn_pin, $runtime));
		let $name = ::core::pin::Pin::into_ref($name) as ::core::pin::Pin<&dyn $crate::unmanaged::UnmanagedSignal<_, _>>;
	};
	{let $name:ident = computed_uncached!($fn_pin:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::unmanaged::computed_uncached($fn_pin, $crate::GlobalSignalsRuntime));
		let $name = ::core::pin::Pin::into_ref($name) as ::core::pin::Pin<&dyn $crate::unmanaged::UnmanagedSignal<_, _>>;
	};
	{let $name:ident = computed_uncached_with_runtime!($fn_pin:expr, $runtime:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::unmanaged::computed_uncached($fn_pin, $runtime));
		let $name = ::core::pin::Pin::into_ref($name) as ::core::pin::Pin<&dyn $crate::unmanaged::UnmanagedSignal<_, _>>;
	};
	{let $name:ident = computed_uncached_mut!($fn_pin:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::unmanaged::computed_uncached_mut($fn_pin, $crate::GlobalSignalsRuntime));
		let $name = ::core::pin::Pin::into_ref($name) as ::core::pin::Pin<&dyn $crate::unmanaged::UnmanagedSignal<_, _>>;
	};
	{let $name:ident = computed_uncached_mut_with_runtime!($fn_pin:expr, $runtime:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::unmanaged::computed_uncached_mut($fn_pin, $runtime));
		let $name = ::core::pin::Pin::into_ref($name) as ::core::pin::Pin<&dyn $crate::unmanaged::UnmanagedSignal<_, _>>;
	};
	{let $name:ident = folded!($init:expr, $fold_fn_pin:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::unmanaged::folded($init, $fold_fn_pin, $crate::GlobalSignalsRuntime));
		let $name = ::core::pin::Pin::into_ref($name) as ::core::pin::Pin<&dyn $crate::unmanaged::UnmanagedSignal<_, _>>;
	};
	{let $name:ident = folded_with_runtime!($init:expr, $fold_fn_pin:expr, $runtime:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::unmanaged::folded($init, $fold_fn_pin, $runtime));
		let $name = ::core::pin::Pin::into_ref($name) as ::core::pin::Pin<&dyn $crate::unmanaged::UnmanagedSignal<_, _>>;
	};
	{let $name:ident = reduced!($select_fn_pin:expr, $reduce_fn_pin:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::unmanaged::reduced($select_fn_pin, $reduce_fn_pin, $crate::GlobalSignalsRuntime));
		let $name = ::core::pin::Pin::into_ref($name) as ::core::pin::Pin<&dyn $crate::unmanaged::UnmanagedSignal<_, _>>;
	};
	{let $name:ident = reduced_with_runtime!($select_fn_pin:expr, $reduce_fn_pin:expr, $runtime:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::unmanaged::reduced($select_fn_pin, $reduce_fn_pin, $runtime));
		let $name = ::core::pin::Pin::into_ref($name) as ::core::pin::Pin<&dyn $crate::unmanaged::UnmanagedSignal<_, _>>;
	};
	{let $name:ident = subscription!($fn_pin:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::__::new_raw_unsubscribed_subscription($crate::unmanaged::computed($fn_pin, $crate::GlobalSignalsRuntime)));
		let $name = ::core::pin::Pin::into_ref($name);
		$crate::__::pull_new_subscription($name);
		let $name = $crate::__::pin_into_pin_impl_source($name);
	};
	{let $name:ident = subscription_with_runtime!($fn_pin:expr, $runtime:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::__::new_raw_unsubscribed_subscription($crate::unmanaged::computed($fn_pin, $runtime)));
		let $name = ::core::pin::Pin::into_ref($name);
		$crate::__::pull_new_subscription($name);
		let $name = $crate::__::pin_into_pin_impl_source($name);
	};
	{let $name:ident = subscription_from_source!($source:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::__::new_raw_unsubscribed_subscription($source));
		let $name = ::core::pin::Pin::into_ref($name);
		$crate::__::pull_new_subscription($name);
		let $name = $crate::__::pin_into_pin_impl_source($name);
	};
	{let $name:ident = effect!($fn_pin:expr, $drop:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::__::new_raw_unsubscribed_effect($fn_pin, $drop, $crate::GlobalSignalsRuntime));
		let $name = ::core::pin::Pin::into_ref($name);
		$name.pull();
	};
	{let $name:ident = effect_with_runtime!($fn_pin:expr, $drop:expr, $runtime:expr$(,)?);} => {
		let $name = ::core::pin::pin!($crate::__::new_raw_unsubscribed_effect($fn_pin, $drop, $runtime));
		let $name = ::core::pin::Pin::into_ref($name);
		$name.pull();
	};
	// Error variant.
	{let $name:ident = $macro:ident!($($arg:expr),*$(,)?);} => {
		// Nicely squiggles the unrecognised identifier… in rust-analyzer.
		// The compiler still squiggles the entire macro, unfortunately.
		::core::compile_error!(::core::concat!(
			"Unrecognised macro name or wrong argument count (for) `", ::core::stringify!($macro), "`. The following macros are supported:\n",
			"inert_cell[_with_runtime]!(1/2), reactive_cell[_mut][_with_runtime]!(2/3), cached!(1), debounced[_with_runtime]!(1/2), ",
			"computed[_uncached[_mut]][_with_runtime]!(1/2), folded[_with_runtime]!(2/3), reduced[_with_runtime]!(2/3), ",
			"subscription[_with_runtime]!(1/2), subscription_from_source!(1), effect[_with_runtime]!(2/3)"
		));
	};
	// Repeat.
	{$(let $name:ident = $macro:ident!($($arg:expr),*$(,)?);)*} => {$(
		$crate::unmanaged::signals_helper! {
			let $name = $macro!($($arg),*);
		}
	)*};
}
pub use crate::signals_helper;
