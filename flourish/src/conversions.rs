//! These *should* be relatively intuitive with regard to Rust's idioms.  
//! Still, navigating the managed handles isn't trivial at first, so here's a set of conversion tables.
//!
//! (Note that these tables are wide and scroll horizontally.)
//!
//! ## with [`UnmanagedSignalCell`]
//!
//! | from ↓ \ into →           | [`&`]‌[`Signal`] ([cell]) | [`&`]‌[`SignalDynCell`]                | [`SignalArc`] ([cell])   | [`SignalArcDynCell`]                          | [`SignalWeak`] ([cell]) | [`SignalWeakDynCell`]                      | [`Subscription`] ([cell])                   | [`SubscriptionDynCell`]                                               |
//! |---------------------------|--------------------------|---------------------------------------|--------------------------|-----------------------------------------------|-------------------------|--------------------------------------------|---------------------------------------------|-----------------------------------------------------------------------|
//! | [`&`]‌[`Signal`] ([cell])  | [identity] + [`Copy`]    | [`.as_dyn_cell()`]                    | [`ToOwned`]              | [`.to_dyn_cell()`]                            | [`.downgrade()`]        | [`.downgrade()`]‌[`.into_dyn_cell()`][idc2] | [`.to_subscription()`]                      | [`.to_subscription()`]‌[`.into_dyn_cell()`][idc3]                      |
//! | [`&`]‌[`SignalDynCell`]    | [identity] + [`Copy`]    | [identity] + [`Copy`]                 | [`ToOwned`]              | [`ToOwned`]                                   | [`.downgrade()`]        | [`.downgrade()`]                           | [`.to_subscription()`]                      | [`.to_subscription()`]                                                |
//! | [`SignalArc`] ([cell])    | [`Deref`] + [`Borrow`]   | [`.as_dyn_cell()`]                    | [identity] + [`Clone`]   | [`.into_dyn_cell()`][idc1]                    | [`.downgrade()`]        | [`.downgrade()`]‌[`.into_dyn_cell()`][idc2] | [`.into_subscription()`]                    | [`.into_subscription()`]‌[`.into_dyn_cell()`][idc3]                    |
//! | [`SignalArcDynCell`]      | [`Deref`] + [`Borrow`]   | [`Deref`] + [`Borrow`]                | [identity] + [`Clone`]   | [identity] + [`Clone`]                        | [`.downgrade()`]        | [`.downgrade()`]                           | [`.into_subscription()`]                    | [`.into_subscription()`]                                              |
//! | [`SignalWeak`] ([cell])   | [`.upgrade()`]‌[`?`]‌      | [`.upgrade()`]‌[`?`]‌[`.as_dyn_cell()`] | [`.upgrade()`]           | [`.upgrade()`]‌[`?`]‌[`.into_dyn_cell()`][idc1] | [identity] + [`Clone`]  | [`.into_dyn_cell()`][idc2]                 | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`] | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`]‌[`.into_dyn_cell()`][idc3] |
//! | [`SignalWeakDynCell`]     | [`.upgrade()`]‌[`?`]‌      | [`.upgrade()`]‌[`?`]‌                   | [`.upgrade()`]           | [`.upgrade()`]                                | [identity] + [`Clone`]  | [identity] + [`Clone`]                     | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`] | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`]                           |
//! | [`Subscription`] ([cell]) | [`Deref`] + [`Borrow`]   | [`.as_dyn_cell()`]                    | [`.unsubscribe()`]       | [`.unsubscribe()`]‌[`.into_dyn_cell()`][idc1]  | [`.downgrade()`]        | [`.downgrade()`]‌[`.into_dyn_cell()`][idc2] | [identity] + [`Clone`]                      | [`.into_dyn_cell()`][idc3]                                            |
//! | [`SubscriptionDynCell`]   | [`Deref`] + [`Borrow`]   | [`Deref`] + [`Borrow`]                | [`.unsubscribe()`]       | [`.unsubscribe()`]                            | [`.downgrade()`]        | [`.downgrade()`]                           | [identity] + [`Clone`]                      | [identity] + [`Clone`]                                                |
//!
//! - In place of [`.as_dyn_cell()`], you can coerce the reference.
//!
//! ## with [`UnmanagedSignal`]
//!
//! | from ↓ \ into →             | [`&`]‌[`Signal`] ([signal]) | [`&`]‌[`SignalDyn`]                 | [`SignalArc`] ([signal]) | [`SignalArcDyn`]                        | [`SignalWeak`] ([signal]) | [`SignalWeakDyn`]                    | [`Subscription`] ([signal])                 | [`SubscriptionDyn`]                                             |
//! |-----------------------------|----------------------------|------------------------------------|--------------------------|-----------------------------------------|---------------------------|--------------------------------------|---------------------------------------------|-----------------------------------------------------------------|
//! | [`&`]‌[`Signal`] ([signal])  | [identity] + [`Copy`]      | [`.as_dyn()`]                      | [`ToOwned`]              | [`.to_dyn()`]                           | [`.downgrade()`]          | [`.downgrade()`]‌[`.into_dyn()`][id2] | [`.to_subscription()`]                      | [`.to_subscription()`]‌[`.into_dyn()`][id3]                      |
//! | [`&`]‌[`SignalDyn`]          | [identity] + [`Copy`]      | [identity] + [`Copy`]              | [`ToOwned`]              | [`ToOwned`]                             | [`.downgrade()`]          | [`.downgrade()`]                     | [`.to_subscription()`]                      | [`.to_subscription()`]                                          |
//! | [`SignalArc`] ([signal])    | [`Deref`] + [`Borrow`]     | [`.as_dyn()`]                      | [identity] + [`Clone`]   | [`.into_dyn()`][id1]                    | [`.downgrade()`]          | [`.downgrade()`]‌[`.into_dyn()`][id2] | [`.into_subscription()`]                    | [`.into_subscription()`]‌[`.into_dyn()`][id3]                    |
//! | [`SignalArcDyn`]            | [`Deref`] + [`Borrow`]     | [`Deref`] + [`Borrow`]             | [identity] + [`Clone`]   | [identity] + [`Clone`]                  | [`.downgrade()`]          | [`.downgrade()`]                     | [`.into_subscription()`]                    | [`.into_subscription()`]                                        |
//! | [`SignalWeak`] ([signal])   | [`.upgrade()`]‌[`?`]‌        | [`.upgrade()`]‌[`?`]‌[`.as_dyn()`]   | [`.upgrade()`]           | [`.upgrade()`]‌[`?`]‌[`.into_dyn()`][id1] | [identity] + [`Clone`]    | [`.into_dyn()`][id2]                 | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`] | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`]‌[`.into_dyn()`][id3] |
//! | [`SignalWeakDyn`]           | [`.upgrade()`]‌[`?`]‌        | [`.upgrade()`]‌[`?`]‌                | [`.upgrade()`]           | [`.upgrade()`]                          | [identity] + [`Clone`]    | [identity] + [`Clone`]               | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`] | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`]                     |
//! | [`Subscription`] ([signal]) | [`Deref`] + [`Borrow`]     | [`.as_dyn()`]                      | [`.unsubscribe()`]       | [`.unsubscribe()`]‌[`.into_dyn()`][id1]  | [`.downgrade()`]          | [`.downgrade()`]‌[`.into_dyn()`][id2] | [identity] + [`Clone`]                      | [`.into_dyn()`][id3]                                            |
//! | [`SubscriptionDyn`]         | [`Deref`] + [`Borrow`]     | [`Deref`] + [`Borrow`]             | [`.unsubscribe()`]       | [`.unsubscribe()`]                      | [`.downgrade()`]          | [`.downgrade()`]                     | [identity] + [`Clone`]                      | [identity] + [`Clone`]                                          |
//!
//! - In place of [`.as_dyn()`], you can coerce the reference.
//!
//! ## [`UnmanagedSignalCell`] to [`UnmanagedSignal`]
//!
//! | from (read-write) ↓ \ into (read-only) → | [`&`]‌[`Signal`] ([signal])             | [`&`]‌[`SignalDyn`]               | [`SignalArc`] ([signal])                       | [`SignalArcDyn`]                        | [`SignalWeak`] ([signal])                   | [`SignalWeakDyn`]                  | [`Subscription`] ([signal])                                          | [`SubscriptionDyn`]                                            |
//! |------------------------------------------|----------------------------------------|----------------------------------|------------------------------------------------|-----------------------------------------|---------------------------------------------|------------------------------------|----------------------------------------------------------------------|----------------------------------------------------------------|
//! | [`&`]‌[`Signal`] ([cell])                 | [`.as_read_only()`]                    | [`.as_dyn()`]                    | [`.to_read_only()`]                            | [`.to_dyn()`]                           | [`.downgrade()`]‌[`.into_read_only()`][iro2] | [`.downgrade()`]‌[.into_dyn()][id2] | [`.to_subscription()`]‌[`.into_read_only()`][iro3]                    | [`.to_subscription()`]‌[`.into_dyn`][id3]                       |
//! | [`SignalArc`] ([cell])                   | [`.as_read_only()`]                    | [`.as_dyn()`]                    | [`.into_read_only()`][iro1]                    | [`.into_dyn()`][id1]                    | [`.downgrade()`]‌[`.into_read_only()`][iro2] | [`.downgrade()`]‌[.into_dyn()][id2] | [`.into_subscription()`]‌[`.into_read_only()`][iro3]                  | [`.into_subscription()`]‌[`.into_dyn`][id3]                     |
//! | [`SignalWeak`] ([cell])                  | [`.upgrade()`]‌[`?`]‌[`.as_read_only()`] | [`.upgrade()`]‌[`?`]‌[`.as_dyn()`] | [`.upgrade()`]‌[`?`]‌[`.into_read_only()`][iro1] | [`.upgrade()`]‌[`?`]‌[`.into_dyn()`][id1] | [`.into_read_only()`][iro2]                 | [.into_dyn()][id2]                 | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`]‌[.into_read_only()][iro3] | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`]‌[.into_dyn()][id3]  |
//! | [`Subscription`] ([cell])                | [`.as_read_only()`]                    | [`.as_dyn()`]                    | [`.unsubscribe()`]‌[`.into_read_only()`][iro1]  | [`.unsubscribe()`]‌[`.into_dyn()`][id1]  | [`.downgrade()`]‌[`.into_read_only()`][iro2] | [`.downgrade()`]‌[.into_dyn()][id2] | [`.into_read_only()`][iro3]                                          | [.into_dyn()][id3]                                             |
//!
//! - In place of [`.as_read_only()`] and [`.as_dyn()`], you can coerce the reference.
//! - `dyn` upcasting conversions can be added as non-breaking change after [`trait_upcasting`](https://github.com/rust-lang/rust/issues/65991) is restabilised.
//!
//! ## [`Effect`]
//!
//! [`Effect`]'s are inconvertible.
//!
//! [cell]: `UnmanagedSignalCell`
//! [identity]: https://doc.rust-lang.org/stable/std/convert/trait.From.html#impl-From%3CT%3E-for-T
//! [`.as_dyn_cell()`]: `Signal::as_dyn_cell`
//! [`.to_dyn_cell()`]: `Signal::to_dyn_cell`
//! [`.downgrade()`]: `Signal::downgrade`
//! [idc1]: `SignalArc::into_dyn_cell`
//! [idc2]: `SignalWeak::into_dyn_cell`
//! [idc3]: `Subscription::into_dyn_cell`
//! [`.into_subscription()`]: `SignalArc::into_subscription`
//! [`.to_subscription()`]: `Signal::to_subscription`
//! [`.unsubscribe()`]: `Subscription::unsubscribe`
//! [`.upgrade()`]: `SignalWeak::upgrade`
//! [`?`]: `core::ops::Try`
//!
//! [signal]: `UnmanagedSignal`
//! [`.as_dyn()`]: `Signal::as_dyn`
//! [`.to_dyn()`]: `Signal::to_dyn`
//! [id1]: `SignalArc::into_dyn`
//! [id2]: `SignalWeak::into_dyn`
//! [id3]: `Subscription::into_dyn`
//!
//! [`.as_read_only()`]: `Signal::as_read_only`
//! [`.to_read_only()`]: `Signal::to_read_only`
//! [iro1]: `SignalArc::into_read_only`
//! [iro2]: `SignalWeak::into_read_only`
//! [iro3]: `Subscription::into_read_only`

#![allow(unused_imports)] // Used by documentation.

use std::{borrow::Borrow, ops::Deref};

use isoprenoid::runtime::SignalsRuntimeRef;

use crate::{
	signal_arc::SignalArcDynCell, traits::UnmanagedSignalCell, unmanaged::UnmanagedSignal, Effect,
	Signal, SignalArc, SignalArcDyn, SignalDyn, SignalDynCell, SignalWeak, SignalWeakDyn,
	SignalWeakDynCell, Subscription, SubscriptionDyn, SubscriptionDynCell,
};

// TODO: `From`/`Into` conversions.

impl<
		'a,
		T: 'a + ?Sized + Send,
		S: 'a + Sized + UnmanagedSignal<T, SR>,
		SR: 'a + ?Sized + SignalsRuntimeRef,
	> From<SignalArc<T, S, SR>> for SignalArcDyn<'a, T, SR>
{
	fn from(value: SignalArc<T, S, SR>) -> Self {
		value.into_dyn()
	}
}

impl<
		'a,
		T: 'a + ?Sized + Send,
		S: 'a + Sized + UnmanagedSignalCell<T, SR>,
		SR: 'a + ?Sized + SignalsRuntimeRef,
	> From<SignalArc<T, S, SR>> for SignalArcDynCell<'a, T, SR>
{
	fn from(value: SignalArc<T, S, SR>) -> Self {
		value.into_dyn_cell()
	}
}

impl<
		'a,
		T: 'a + ?Sized + Send,
		S: 'a + Sized + UnmanagedSignal<T, SR>,
		SR: 'a + ?Sized + SignalsRuntimeRef,
	> From<SignalWeak<T, S, SR>> for SignalWeakDyn<'a, T, SR>
{
	fn from(value: SignalWeak<T, S, SR>) -> Self {
		value.into_dyn()
	}
}

impl<
		'a,
		T: 'a + ?Sized + Send,
		S: 'a + Sized + UnmanagedSignalCell<T, SR>,
		SR: 'a + ?Sized + SignalsRuntimeRef,
	> From<SignalWeak<T, S, SR>> for SignalWeakDynCell<'a, T, SR>
{
	fn from(value: SignalWeak<T, S, SR>) -> Self {
		value.into_dyn_cell()
	}
}

impl<
		'a,
		T: 'a + ?Sized + Send,
		S: 'a + Sized + UnmanagedSignal<T, SR>,
		SR: 'a + ?Sized + SignalsRuntimeRef,
	> From<Subscription<T, S, SR>> for SubscriptionDyn<'a, T, SR>
{
	fn from(value: Subscription<T, S, SR>) -> Self {
		value.into_dyn()
	}
}

impl<
		'a,
		T: 'a + ?Sized + Send,
		S: 'a + Sized + UnmanagedSignalCell<T, SR>,
		SR: 'a + ?Sized + SignalsRuntimeRef,
	> From<Subscription<T, S, SR>> for SubscriptionDynCell<'a, T, SR>
{
	fn from(value: Subscription<T, S, SR>) -> Self {
		value.into_dyn_cell()
	}
}

//TODO: Conversion from UnmanagedSignalCell.
