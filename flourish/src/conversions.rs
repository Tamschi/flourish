//! # Overview of conversions
//!
//! This module implements conversions between various types in this crate.
//!
//! ## [`From`] conversions
//!
//! Where a conversion is available and not the identity conversion, the following tables
//! list the respective type-associated convenience method that can be used instead of [`Into::into`].
//!
//! `→` indicates that the conversion is available only to a subset of the target type, one cell to the right.  
//! Entries prefixed with '(`&`)' convert from references or borrow instead of consuming the handle.
//!
//! **Macro authors should use qualified [`From`] and [`Into`] conversions instead of duck-typing the static-dispatch API.**
//!
//! Note that only side-effect-free conversions are supported via [`From`]:
//!
//! ## with [`UnmanagedSignalCell`]
//!
//! | from ↓ \ into →           | [`&`]‌[`Signal`] ([cell]) | [`&`]‌[`SignalDynCell`]             | [`SignalArc`] ([cell])   | [`SignalArcDynCell`]                  | [`SignalWeak`] ([cell]) | [`SignalWeakDynCell`]                 | [`Subscription`] ([cell]) | [`SubscriptionDynCell`]             |
//! |---------------------------|--------------------------|------------------------------------|--------------------------|---------------------------------------|-------------------------|---------------------------------------|---------------------------|-------------------------------------|
//! | [`&`]‌[`Signal`] ([cell])  | [identity] + [`Copy`]    | coercion / [`.as_dyn_cell()`]      | [`ToOwned`]              | via [`SignalArc`]                     | [`.downgrade()`]        | via [`SignalWeak`]                    | [`.to_subscription()`]    | via [`Subscription`]                |
//! | [`&`]‌[`SignalDynCell`]    | [identity] + [`Copy`]    | [identity] + [`Copy`]              | [`ToOwned`]              | [`ToOwned`]                           | [`.downgrade()`]        | [`.downgrade()`]                      | [`.to_subscription()`]    | [`.to_subscription()`]              |
//! | [`SignalArc`] ([cell])    | [`Deref`]                | via [`&`]‌[`Signal`]                | [identity] + [`Clone`]   | coercion / [`.into_dyn_cell()`][idc1] | via [`&`]‌[`Signal`]     | via [`&`]‌[`Signal`], [`SignalWeak`]   | [`.into_subscription()`]  | via [`Subscription`]                |
//! | [`SignalArcDynCell`]      | [`Deref`]                | [`Deref`]                          | [identity] + [`Clone`]   | [identity] + [`Clone`]                | via [`&`]‌[`Signal`]     | via [`&`]‌[`Signal`]                   | [`.into_subscription()`]  | [`.into_subscription()`]            |
//! | [`SignalWeak`] ([cell])   | via [`SignalArc`]        | via [`SignalArc`], [`&`]‌[`Signal`] | [`.upgrade()`]           | via [`SignalArc`]                     | [identity] + [`Clone`]  | coercion / [`.into_dyn_cell()`][idc2] | via [`SignalArc`]         | via [`SignalArc`], [`Subscription`] |
//! | [`SignalWeakDynCell`]     | via [`SignalArc`]        | via [`SignalArc`]                  | [`.upgrade()`]           | [`.upgrade()`]                        | [identity] + [`Clone`]  | [identity] + [`Clone`]                | via [`SignalArc`]         | via [`SignalArc`]                   |
//! | [`Subscription`] ([cell]) | [`Deref`]                | via [`&`]‌[`Signal`]                | [`.unsubscribe()`]       | via [`SignalArc`]                     | via [`&`]‌[`Signal`]     | via [`&`]‌[`Signal`], [`SignalWeak`]   | [identity] + [`Clone`]    | [`.into_dyn_cell()`][idc3]          |
//! | [`SubscriptionDynCell`]   | [`Deref`]                | [`Deref`]                          | [`.unsubscribe()`]       | [`.unsubscribe()`]                    | via [`&`]‌[`Signal`]     | via [`&`]‌[`Signal`]                   | [identity] + [`Clone`]    | [identity] + [`Clone`]              |
//!
//! ## with [`UnmanagedSignal`]
//!
//! | from ↓ \ into →             | [`&`]‌[`Signal`] ([signal]) | [`&`]‌[`SignalDyn`]                 | [`SignalArc`] ([signal]) | [`SignalArcDyn`]                | [`SignalWeak`] ([signal]) | [`SignalWeakDyn`]                   | [`Subscription`] ([signal]) | [`SubscriptionDyn`]                 |
//! |-----------------------------|----------------------------|------------------------------------|--------------------------|---------------------------------|---------------------------|-------------------------------------|-----------------------------|-------------------------------------|
//! | [`&`]‌[`Signal`] ([signal])  | [identity] + [`Copy`]      | coercion / [`.as_dyn()`]           | [`ToOwned`]              | via [`SignalArc`]               | [`.downgrade()`]          | via [`SignalWeak`]                  | [`.to_subscription()`]      | via [`Subscription`]                |
//! | [`&`]‌[`SignalDyn`]          | [identity] + [`Copy`]      | [identity] + [`Copy`]              | [`ToOwned`]              | [`ToOwned`]                     | [`.downgrade()`]          | [`.downgrade()`]                    | [`.to_subscription()`]      | [`.to_subscription()`]              |
//! | [`SignalArc`] ([signal])    | [`Deref`]                  | via [`&`]‌[`Signal`]                | [identity] + [`Clone`]   | coercion / [`.into_dyn()`][id1] | via [`&`]‌[`Signal`]       | via [`&`]‌[`Signal`], [`SignalWeak`] | [`.into_subscription()`]    | via [`Subscription`]                |
//! | [`SignalArcDyn`]            | [`Deref`]                  | [`Deref`]                          | [identity] + [`Clone`]   | [identity] + [`Clone`]          | via [`&`]‌[`Signal`]       | via [`&`]‌[`Signal`]                 | [`.into_subscription()`]    | [`.into_subscription()`]            |
//! | [`SignalWeak`] ([signal])   | via [`SignalArc`]          | via [`SignalArc`], [`&`]‌[`Signal`] | [`.upgrade()`]           | via [`SignalArc`]               | [identity] + [`Clone`]    | coercion / [`.into_dyn()`][id2]     | via [`SignalArc`]           | via [`SignalArc`], [`Subscription`] |
//! | [`SignalWeakDyn`]           | via [`SignalArc`]          | via [`SignalArc`]                  | [`.upgrade()`]           | [`.upgrade()`]                  | [identity] + [`Clone`]    | [identity] + [`Clone`]              | via [`SignalArc`]           | via [`SignalArc`]                   |
//! | [`Subscription`] ([signal]) | [`Deref`]                  | via [`&`]‌[`Signal`]                | [`.unsubscribe()`]       | via [`SignalArc`]               | via [`&`]‌[`Signal`]       | via [`&`]‌[`Signal`], [`SignalWeak`] | [identity] + [`Clone`]      | [`.into_dyn()`][id3]                |
//! | [`SubscriptionDyn`]         | [`Deref`]                  | [`Deref`]                          | [`.unsubscribe()`]       | [`.unsubscribe()`]              | via [`&`]‌[`Signal`]       | via [`&`]‌[`Signal`]                 | [identity] + [`Clone`]      | [identity] + [`Clone`]              |
//!
//! ## [`UnmanagedSignalCell`] to [`UnmanagedSignal`]
//!
//! | from (read-write) ↓ \ into (read-only) → | [`&`]‌[`Signal`] ([signal])   | [`&`]‌[`SignalDyn`]               | [`SignalArc`] ([signal])               | [`SignalArcDyn`]                | [`SignalWeak`] ([signal])                             | [`SignalWeakDyn`]                                     | [`Subscription`] ([signal])   | [`SubscriptionDyn`]           |
//! |------------------------------------------|------------------------------|----------------------------------|----------------------------------------|---------------------------------|-------------------------------------------------------|-------------------------------------------------------|-------------------------------|-------------------------------|
//! | [`&`]‌[`Signal`] ([cell])                 | [`.as_read_only()`]          | coercion / [`.as_dyn()`]         | via [`SignalArc`] ([cell])             | via [`SignalArc`] ([cell])      | via [`SignalWeak`] ([cell])                           | via [`SignalWeak`] ([cell])                           | via [`Subscription`] ([cell]) | via [`Subscription`] ([cell]) |
//! | [`SignalArc`] ([cell])                   | via [`&`]‌[`Signal`] ([cell]) | via [`&`]‌[`Signal`] ([cell])     | coercion / [`.into_read_only()`][iro1] | coercion / [`.into_dyn()`][id1] | via [`&`]‌[`Signal`] ([cell]), [`SignalWeak`] ([cell]) | via [`&`]‌[`Signal`] ([cell]), [`SignalWeak`] ([cell]) | via [`Subscription`] ([cell]) | via [`Subscription`] ([cell]) |
//! | [`SignalWeak`] ([cell])                  | TODO
//! | [`Subscription`] ([cell])                | TODO
//!
//! [`.as_dyn_cell()`]: `Signal::as_dyn_cell`
//! [`.downgrade()`]: `Signal::downgrade`
//! [`.to_subscription()`]: `Signal::to_subscription`
//! [idc1]: `SignalArc::into_dyn_cell`
//! [`.into_subscription()`]: `SignalArc::into_subscription`
//! [`.upgrade()`]: `SignalWeak::upgrade`
//! [`.unsubscribe()`]: `Subscription::unsubscribe`
//! [idc2]: `SignalWeak::into_dyn_cell`
//! [idc3]: `Subscription::into_dyn_cell`
//!
//! [signal]: `UnmanagedSignal`
//! [`.as_dyn()`]: `Signal::as_dyn`
//! [id1]: `SignalArc::into_dyn`
//! [id2]: `SignalWeak::into_dyn`
//! [id3]: `Subscription::into_dyn`
//!
//! [`.as_read_only()`]: `Signal::as_read_only`
//! [iro1]: `SignalArc::into_read_only`
//! [iro2]: `SignalWeak::into_read_only`
//! [iro3]: `Subscription::into_read_only`
//!
//! //TODO: Formatting!
//! //TODO: Table for subscriptions.
//! //TODO: Note that `Effects` aren't convertible.
//!
//! //TODO: On second thought, remove most of the convenience methods, implement [`Borrow`], [`ToOwned`], [`Deref`] and possibly [`AsRef`] instead.
//! //      (Refcounting handles can wrap Refs!)
//!
//! [cell]: `UnmanagedSignalCell`
//! [identity]: https://doc.rust-lang.org/stable/std/convert/trait.From.html#impl-From%3CT%3E-for-T
//! [c1]: `SignalCellRef::clone`
//! [id1]: `SignalCellSR::into_dyn`
//! [cd1]: `SignalCellRef::clone_dyn`
//! [ar1]: `SignalCellSR::as_ref`
//! [ard1]: `SignalCellSR::as_ref_dyn`
//! [id2]: `SignalCellRef::into_dyn`
//!
//! [is1]: `SignalCellSR::into_signal`
//! [ts1]: `SignalCellSR::to_signal`
//! [ts2]: `SignalCellRef::to_signal`
//! [c2]: `SignalRef::clone`
//! [isd1]: `SignalCellSR::into_signal_dyn`
//! [tsd1]: `SignalCellSR::to_signal_dyn`
//! [tsd2]: `SignalCellRef::to_signal_dyn`
//! [id3]: `SignalSR::into_dyn`
//! [cd2]: `SignalRef::clone_dyn`
//! [asr1]: `SignalCellSR::as_signal_ref`
//! [isr1]: `SignalCellSR::as_signal_ref`
//! [ar2]: `SignalSR::as_ref`
//! [asrd1]: `SignalCellSR::as_signal_ref_dyn`
//! [isrd1]: `SignalCellSR::into_signal_ref_dyn`
//! [ard2]: `SignalSR::as_ref_dyn`
//! [id4]: `SignalRef::into_dyn`
//!
//! Special cases like [`Signal`](`crate::Signal`) of [`SignalSR`] are omitted for clarity.
//!
//! Entries that say '`.into_dyn()`' should be upgradable to unsizing coercions eventually.
//!
//! Each [`ArcSourcePin`] above has an associated [`WeakSourcePin`] with equivalent conversions:
//! Types can be converted among themselves just like their strong variant, but up- and downgrades
//! must be explicit.
//!
//! ## Side-effect conversions

use std::ops::Deref;

use isoprenoid::runtime::SignalsRuntimeRef;

use crate::{
	signal_arc::SignalArcDynCell, traits::UnmanagedSignalCell, unmanaged::UnmanagedSignal, Signal,
	SignalArc, SignalArcDyn, SignalDyn, SignalDynCell, SignalWeak, SignalWeakDyn,
	SignalWeakDynCell, Subscription, SubscriptionDyn, SubscriptionDynCell,
};

// into `SignalCellRefDyn`

// TODO

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

//TODO: Conversion from UnmanagedSignalCell.
