//! These *should* be relatively intuitive with regard to Rust's idioms.  
//! Still, navigating the managed handles isn't trivial at first, so here's a set of conversion tables.
//!
//! (Note that these tables are wide and scroll horizntally.)
//!
//! ## with [`UnmanagedSignalCell`]
//!
//! | from ↓ \ into →           | [`&`]‌[`Signal`] ([cell]) | [`&`]‌[`SignalDynCell`]                | [`SignalArc`] ([cell])   | [`SignalArcDynCell`]                          | [`SignalWeak`] ([cell]) | [`SignalWeakDynCell`]                      | [`Subscription`] ([cell])                   | [`SubscriptionDynCell`]                                               |
//! |---------------------------|--------------------------|---------------------------------------|--------------------------|-----------------------------------------------|-------------------------|--------------------------------------------|---------------------------------------------|-----------------------------------------------------------------------|
//! | [`&`]‌[`Signal`] ([cell])  | [identity] + [`Copy`]    | [`.as_dyn_cell()`]                    | [`ToOwned`]              | [`.to_dyn()`]                                 | [`.downgrade()`]        | [`.downgrade()`]‌[`.into_dyn_cell()`][idc2] | [`.to_subscription()`]                      | [`.to_subscription()`]‌[`.into_dyn_cell()`][idc3]                      |
//! | [`&`]‌[`SignalDynCell`]    | [identity] + [`Copy`]    | [identity] + [`Copy`]                 | [`ToOwned`]              | [`ToOwned`]                                   | [`.downgrade()`]        | [`.downgrade()`]                           | [`.to_subscription()`]                      | [`.to_subscription()`]                                                |
//! | [`SignalArc`] ([cell])    | [`Deref`]                | [`.as_dyn_cell()`]                    | [identity] + [`Clone`]   | [`.into_dyn_cell()`][idc1]                    | [`.downgrade()`]        | [`.downgrade()`]‌[`.into_dyn_cell()`][idc2] | [`.into_subscription()`]                    | [`.into_subscription()`]‌[`.into_dyn_cell()`][idc3]                    |
//! | [`SignalArcDynCell`]      | [`Deref`]                | [`Deref`]                             | [identity] + [`Clone`]   | [identity] + [`Clone`]                        | [`.downgrade()`]        | [`.downgrade()`]                           | [`.into_subscription()`]                    | [`.into_subscription()`]                                              |
//! | [`SignalWeak`] ([cell])   | [`.upgrade()`]‌[`?`]‌      | [`.upgrade()`]‌[`?`]‌[`.as_dyn_cell()`] | [`.upgrade()`]           | [`.upgrade()`]‌[`?`]‌[`.into_dyn_cell()`][idc1] | [identity] + [`Clone`]  | [`.into_dyn_cell()`][idc2]                 | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`] | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`]‌[`.into_dyn_cell()`][idc3] |
//! | [`SignalWeakDynCell`]     | [`.upgrade()`]‌[`?`]‌      | [`.upgrade()`]‌[`?`]‌                   | [`.upgrade()`]           | [`.upgrade()`]                                | [identity] + [`Clone`]  | [identity] + [`Clone`]                     | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`] | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`]                           |
//! | [`Subscription`] ([cell]) | [`Deref`]                | [`.as_dyn_cell()`]                    | [`.unsubscribe()`]       | [`.unsubscribe()`]‌[`.into_dyn_cell()`][idc1]  | [`.downgrade()`]        | [`.downgrade()`]‌[`.into_dyn_cell()`][idc2] | [identity] + [`Clone`]                      | [`.into_dyn_cell()`][idc3]                                            |
//! | [`SubscriptionDynCell`]   | [`Deref`]                | [`Deref`]                             | [`.unsubscribe()`]       | [`.unsubscribe()`]                            | [`.downgrade()`]        | [`.downgrade()`]                           | [identity] + [`Clone`]                      | [identity] + [`Clone`]                                                |
//!
//! - In place of [`.as_dyn_cell()`], you can coerce the reference.
//! - In place of `.into_dyn_cell()` ([1][idc1], [2][idc2], [3][idc3]), you can coerce the value.
//!
//! ## with [`UnmanagedSignal`]
//!
//! | from ↓ \ into →             | [`&`]‌[`Signal`] ([signal]) | [`&`]‌[`SignalDyn`]                 | [`SignalArc`] ([signal]) | [`SignalArcDyn`]                        | [`SignalWeak`] ([signal]) | [`SignalWeakDyn`]                    | [`Subscription`] ([signal])                 | [`SubscriptionDyn`]                                             |
//! |-----------------------------|----------------------------|------------------------------------|--------------------------|-----------------------------------------|---------------------------|--------------------------------------|---------------------------------------------|-----------------------------------------------------------------|
//! | [`&`]‌[`Signal`] ([signal])  | [identity] + [`Copy`]      | [`.as_dyn()`]                      | [`ToOwned`]              | [`.to_dyn()`]                           | [`.downgrade()`]          | [`.downgrade()`]‌[`.into_dyn()`][id2] | [`.to_subscription()`]                      | [`.to_subscription()`]‌[`.into_dyn()`][id3]                      |
//! | [`&`]‌[`SignalDyn`]          | [identity] + [`Copy`]      | [identity] + [`Copy`]              | [`ToOwned`]              | [`ToOwned`]                             | [`.downgrade()`]          | [`.downgrade()`]                     | [`.to_subscription()`]                      | [`.to_subscription()`]                                          |
//! | [`SignalArc`] ([signal])    | [`Deref`]                  | [`.as_dyn()`]                      | [identity] + [`Clone`]   | [`.into_dyn()`][id1]                    | [`.downgrade()`]          | [`.downgrade()`]‌[`.into_dyn()`][id2] | [`.into_subscription()`]                    | [`.into_subscription()`]‌[`.into_dyn()`][id3]                    |
//! | [`SignalArcDyn`]            | [`Deref`]                  | [`Deref`]                          | [identity] + [`Clone`]   | [identity] + [`Clone`]                  | [`.downgrade()`]          | [`.downgrade()`]                     | [`.into_subscription()`]                    | [`.into_subscription()`]                                        |
//! | [`SignalWeak`] ([signal])   | [`.upgrade()`]‌[`?`]‌        | [`.upgrade()`]‌[`?`]‌[`.as_dyn()`]   | [`.upgrade()`]           | [`.upgrade()`]‌[`?`]‌[`.into_dyn()`][id1] | [identity] + [`Clone`]    | [`.into_dyn()`][id2]                 | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`] | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`]‌[`.into_dyn()`][id3] |
//! | [`SignalWeakDyn`]           | [`.upgrade()`]‌[`?`]‌        | [`.upgrade()`]‌[`?`]‌                | [`.upgrade()`]           | [`.upgrade()`]                          | [identity] + [`Clone`]    | [identity] + [`Clone`]               | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`] | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`]                     |
//! | [`Subscription`] ([signal]) | [`Deref`]                  | [`.as_dyn()`]                      | [`.unsubscribe()`]       | [`.unsubscribe()`]‌[`.into_dyn()`][id1]  | [`.downgrade()`]          | [`.downgrade()`]‌[`.into_dyn()`][id2] | [identity] + [`Clone`]                      | [`.into_dyn()`][id3]                                            |
//! | [`SubscriptionDyn`]         | [`Deref`]                  | [`Deref`]                          | [`.unsubscribe()`]       | [`.unsubscribe()`]                      | [`.downgrade()`]          | [`.downgrade()`]                     | [identity] + [`Clone`]                      | [identity] + [`Clone`]                                          |
//!
//! - In place of [`.as_dyn()`], you can coerce the reference.
//! - In place of `.into_dyn()` ([1][id1], [2][id2], [3][id3]), you can coerce the value.
//!
//! ## [`UnmanagedSignalCell`] to [`UnmanagedSignal`]
//!
//! | from (read-write) ↓ \ into (read-only) → | [`&`]‌[`Signal`] ([signal])             | [`&`]‌[`SignalDyn`]               | [`SignalArc`] ([signal])                       | [`SignalArcDyn`]                        | [`SignalWeak`] ([signal])                   | [`SignalWeakDyn`]                  | [`Subscription`] ([signal])                                          | [`SubscriptionDyn`]                                            |
//! |------------------------------------------|----------------------------------------|----------------------------------|------------------------------------------------|-----------------------------------------|---------------------------------------------|------------------------------------|----------------------------------------------------------------------|----------------------------------------------------------------|
//! | [`&`]‌[`Signal`] ([cell])                 | [`.as_read_only()`]                    | [`.as_dyn()`]                    | [`.to_read_only()`]                            | [`.to_dyn()`]                           | [`.downgrade()`]‌[`.into_read_only()`][iro2] | [`.downgrade()`]‌[.into_dyn()][id2] | [`.to_subscription()`]‌[`.into_read_only()`][iro3]                    | [`.to_subscription()`]‌[`.into_dyn`][id3]                       |
//! | [`SignalArc`] ([cell])                   | [`.as_read_only()`]                    | [`.as_dyn()`]                    | [`.into_read_only()`][iro1]                    | [`.into_dyn()`][id1]                    | [`.downgrade()`]‌[`.into_read_only()`][iro2] | [`.downgrade()`]‌[.into_dyn()][id2] | [`.into_subscription()`]‌[`.into_read_only()`][iro3]                  | [`.into_subscription()`]‌[`.into_dyn`][id3]                     |
//! | [`SignalWeak`] ([cell])                  | [`.upgrade()`]‌[`?`]‌[`.as_read_only()`] | [`.upgrade()`]‌[`?`]‌[`.as_dyn()`] | [`.upgrade()`]‌[`?`]‌[`.into_read_only()`][iro1] | [`.upgrade()`]‌[`?`]‌[`.into_dyn()`][id1] | [`.into_read_only()`][iro2]                 | [.into_dyn()][id2]                 | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`]‌[.into_read_only()][iro3] | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`]‌[.into_dyn()][id3]  |
//! | [`Subscription`] ([cell])                | [`.as_read_only()`]                    | [`.as_dyn()`]                    | [.unsubscribe()]‌[`.into_read_only()`][iro1]    | [.unsubscribe()]‌[`.into_dyn()`][id1]    | [`.downgrade()`]‌[`.into_read_only()`][iro2] | [`.downgrade()`]‌[.into_dyn()][id2] | [`.into_read_only()`][iro3]                                          | [.into_dyn()][id3]                                             |
//! 
//! - In place of [`.as_read_only()`] and [`.as_dyn()`], you can coerce the reference.
//! - In place of `.into_read_only()` ([1][iro1], [2][iro2], [3][iro3]) and `.into_dyn()` ([1][id1], [2][id2], [3][id3]), you can coerce the value.
//!
//! [`.as_dyn_cell()`]: `Signal::as_dyn_cell`
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
//! [id1]: `SignalArc::into_dyn`
//! [id2]: `SignalWeak::into_dyn`
//! [id3]: `Subscription::into_dyn`
//!
//! [`.as_read_only()`]: `Signal::as_read_only`
//! [`.to_read_only()`]: `Signal::to_read_only`
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
