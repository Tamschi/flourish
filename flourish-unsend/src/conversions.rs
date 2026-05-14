//! Conversions between this library's signal-related types.
//! (Documentation module.)
//!
//! (Note that these tables are wide and scroll horizontally.)
//!
//! ## with [`UnmanagedSignalCell`]
//!
//! | from ↓ \ into →           | [`&`]‌[`Signal`] ([cell]) | [`&`]‌[`SignalDynCell`]                | [`SignalRc`] ([cell])   | [`SignalRcDynCell`]                          | [`SignalWeak`] ([cell]) | [`SignalWeakDynCell`]                      | [`Subscription`] ([cell])                   | [`SubscriptionDynCell`]                                               |
//! |---------------------------|--------------------------|---------------------------------------|--------------------------|-----------------------------------------------|-------------------------|--------------------------------------------|---------------------------------------------|-----------------------------------------------------------------------|
//! | [`&`]‌[`Signal`] ([cell])  | [identity] + [`Copy`]    | [`.as_dyn_cell()`]                    | [`ToOwned`]              | [`.to_dyn_cell()`]                            | [`.downgrade()`]        | [`.downgrade()`]‌[`.into_dyn_cell()`][idc2] | [`.to_subscription()`]                      | [`.to_subscription()`]‌[`.into_dyn_cell()`][idc3]                      |
//! | [`&`]‌[`SignalDynCell`]    | [identity] + [`Copy`]    | [identity] + [`Copy`]                 | [`ToOwned`]              | [`ToOwned`]                                   | [`.downgrade()`]        | [`.downgrade()`]                           | [`.to_subscription()`]                      | [`.to_subscription()`]                                                |
//! | [`SignalRc`] ([cell])    | [`Deref`] + [`Borrow`]   | [`.as_dyn_cell()`]                    | [identity] + [`Clone`]   | [`.into_dyn_cell()`][idc1]                    | [`.downgrade()`]        | [`.downgrade()`]‌[`.into_dyn_cell()`][idc2] | [`.into_subscription()`]                    | [`.into_subscription()`]‌[`.into_dyn_cell()`][idc3]                    |
//! | [`SignalRcDynCell`]      | [`Deref`] + [`Borrow`]   | [`Deref`] + [`Borrow`]                | [identity] + [`Clone`]   | [identity] + [`Clone`]                        | [`.downgrade()`]        | [`.downgrade()`]                           | [`.into_subscription()`]                    | [`.into_subscription()`]                                              |
//! | [`SignalWeak`] ([cell])   | [`.upgrade()`]‌[`?`]‌      | [`.upgrade()`]‌[`?`]‌[`.as_dyn_cell()`] | [`.upgrade()`]           | [`.upgrade()`]‌[`?`]‌[`.into_dyn_cell()`][idc1] | [identity] + [`Clone`]  | [`.into_dyn_cell()`][idc2]                 | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`] | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`]‌[`.into_dyn_cell()`][idc3] |
//! | [`SignalWeakDynCell`]     | [`.upgrade()`]‌[`?`]‌      | [`.upgrade()`]‌[`?`]‌                   | [`.upgrade()`]           | [`.upgrade()`]                                | [identity] + [`Clone`]  | [identity] + [`Clone`]                     | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`] | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`]                           |
//! | [`Subscription`] ([cell]) | [`Deref`] + [`Borrow`]   | [`.as_dyn_cell()`]                    | [`.unsubscribe()`]       | [`.unsubscribe()`]‌[`.into_dyn_cell()`][idc1]  | [`.downgrade()`]        | [`.downgrade()`]‌[`.into_dyn_cell()`][idc2] | [identity] + [`Clone`]                      | [`.into_dyn_cell()`][idc3]                                            |
//! | [`SubscriptionDynCell`]   | [`Deref`] + [`Borrow`]   | [`Deref`] + [`Borrow`]                | [`.unsubscribe()`]       | [`.unsubscribe()`]                            | [`.downgrade()`]        | [`.downgrade()`]                           | [identity] + [`Clone`]                      | [identity] + [`Clone`]                                                |
//!
//! - In place of [`.as_dyn_cell()`], you can coerce the reference.
//!
//! ## with [`UnmanagedSignal`]
//!
//! | from ↓ \ into →             | [`&`]‌[`Signal`] ([signal]) | [`&`]‌[`SignalDyn`]                 | [`SignalRc`] ([signal]) | [`SignalRcDyn`]                        | [`SignalWeak`] ([signal]) | [`SignalWeakDyn`]                    | [`Subscription`] ([signal])                 | [`SubscriptionDyn`]                                             |
//! |-----------------------------|----------------------------|------------------------------------|--------------------------|-----------------------------------------|---------------------------|--------------------------------------|---------------------------------------------|-----------------------------------------------------------------|
//! | [`&`]‌[`Signal`] ([signal])  | [identity] + [`Copy`]      | [`.as_dyn()`]                      | [`ToOwned`]              | [`.to_dyn()`]                           | [`.downgrade()`]          | [`.downgrade()`]‌[`.into_dyn()`][id2] | [`.to_subscription()`]                      | [`.to_subscription()`]‌[`.into_dyn()`][id3]                      |
//! | [`&`]‌[`SignalDyn`]          | [identity] + [`Copy`]      | [identity] + [`Copy`]              | [`ToOwned`]              | [`ToOwned`]                             | [`.downgrade()`]          | [`.downgrade()`]                     | [`.to_subscription()`]                      | [`.to_subscription()`]                                          |
//! | [`SignalRc`] ([signal])    | [`Deref`] + [`Borrow`]     | [`.as_dyn()`]                      | [identity] + [`Clone`]   | [`.into_dyn()`][id1]                    | [`.downgrade()`]          | [`.downgrade()`]‌[`.into_dyn()`][id2] | [`.into_subscription()`]                    | [`.into_subscription()`]‌[`.into_dyn()`][id3]                    |
//! | [`SignalRcDyn`]            | [`Deref`] + [`Borrow`]     | [`Deref`] + [`Borrow`]             | [identity] + [`Clone`]   | [identity] + [`Clone`]                  | [`.downgrade()`]          | [`.downgrade()`]                     | [`.into_subscription()`]                    | [`.into_subscription()`]                                        |
//! | [`SignalWeak`] ([signal])   | [`.upgrade()`]‌[`?`]‌        | [`.upgrade()`]‌[`?`]‌[`.as_dyn()`]   | [`.upgrade()`]           | [`.upgrade()`]‌[`?`]‌[`.into_dyn()`][id1] | [identity] + [`Clone`]    | [`.into_dyn()`][id2]                 | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`] | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`]‌[`.into_dyn()`][id3] |
//! | [`SignalWeakDyn`]           | [`.upgrade()`]‌[`?`]‌        | [`.upgrade()`]‌[`?`]‌                | [`.upgrade()`]           | [`.upgrade()`]                          | [identity] + [`Clone`]    | [identity] + [`Clone`]               | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`] | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`]                     |
//! | [`Subscription`] ([signal]) | [`Deref`] + [`Borrow`]     | [`.as_dyn()`]                      | [`.unsubscribe()`]       | [`.unsubscribe()`]‌[`.into_dyn()`][id1]  | [`.downgrade()`]          | [`.downgrade()`]‌[`.into_dyn()`][id2] | [identity] + [`Clone`]                      | [`.into_dyn()`][id3]                                            |
//! | [`SubscriptionDyn`]         | [`Deref`] + [`Borrow`]     | [`Deref`] + [`Borrow`]             | [`.unsubscribe()`]       | [`.unsubscribe()`]                      | [`.downgrade()`]          | [`.downgrade()`]                     | [identity] + [`Clone`]                      | [identity] + [`Clone`]                                          |
//!
//! - In place of [`.as_dyn()`], you can coerce the reference.
//!
//! ## with [`UnmanagedSignalCell`] (static dispatch read-write) to with [`UnmanagedSignal`] (read-only)
//!
//! | from ↓ \ into →           | [`&`]‌[`Signal`] ([signal])                   | [`&`]‌[`SignalDyn`]               | [`SignalRc`] ([signal])                       | [`SignalRcDyn`]                        | [`SignalWeak`] ([signal])                   | [`SignalWeakDyn`]                  | [`Subscription`] ([signal])                                              | [`SubscriptionDyn`]                                             |
//! |---------------------------|----------------------------------------------|----------------------------------|------------------------------------------------|-----------------------------------------|---------------------------------------------|------------------------------------|--------------------------------------------------------------------------|-----------------------------------------------------------------|
//! | [`&`]‌[`Signal`] ([cell])  | [`.as_read_only()`][aro1]                    | [`.as_dyn()`]                    | [`.to_read_only()`][tro1]                      | [`.to_dyn()`]                           | [`.downgrade()`]‌[`.into_read_only()`][iro2] | [`.downgrade()`]‌[`.into_dyn()`][id2] | [`.to_subscription()`]‌[`.into_read_only()`][iro3]                      | [`.to_subscription()`]‌[`.into_dyn`][id3]                        |
//! | [`SignalRc`] ([cell])    | [`.as_read_only()`][aro1]                    | [`.as_dyn()`]                    | [`.into_read_only()`][iro1]                    | [`.into_dyn()`][id1]                    | [`.downgrade()`]‌[`.into_read_only()`][iro2] | [`.downgrade()`]‌[`.into_dyn()`][id2] | [`.into_subscription()`]‌[`.into_read_only()`][iro3]                    | [`.into_subscription()`]‌[`.into_dyn`][id3]                      |
//! | [`SignalWeak`] ([cell])   | [`.upgrade()`]‌[`?`]‌[`.as_read_only()`][aro1] | [`.upgrade()`]‌[`?`]‌[`.as_dyn()`] | [`.upgrade()`]‌[`?`]‌[`.into_read_only()`][iro1] | [`.upgrade()`]‌[`?`]‌[`.into_dyn()`][id1] | [`.into_read_only()`][iro2]                 | [`.into_dyn()`][id2]                 | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`]‌[`.into_read_only()`][iro3] | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`]‌[`.into_dyn()`][id3] |
//! | [`Subscription`] ([cell]) | [`.as_read_only()`][aro1]                    | [`.as_dyn()`]                    | [`.unsubscribe()`]‌[`.into_read_only()`][iro1]  | [`.unsubscribe()`]‌[`.into_dyn()`][id1]  | [`.downgrade()`]‌[`.into_read_only()`][iro2] | [`.downgrade()`]‌[`.into_dyn()`][id2] | [`.into_read_only()`][iro3]                                            | [`.into_dyn()`][id3]                                            |
//!
//! - In place of [`.as_read_only()`][aro1] and [`.as_dyn()`], you can coerce the reference.
//!
//! ## with `dyn `[`UnmanagedSignalCell`] (read-write) to with `dyn `[`UnmanagedSignal`] (read-only)
//!
//! | from ↓ \ into →         | [`&`]‌[`SignalDyn`]                           | [`SignalRcDyn`]                               | [`SignalWeakDyn`]                           | [`SubscriptionDyn`]                                                    |
//! |-------------------------|----------------------------------------------|------------------------------------------------|---------------------------------------------|------------------------------------------------------------------------|
//! | [`&`]‌[`SignalDynCell`]  | [`.as_read_only()`][aro2]                    | [`.to_read_only()`][tro2]                      | [`.downgrade()`]‌[`.into_read_only()`][iro5] | [`.to_subscription()`]‌[`.into_read_only()`][iro6]                      |
//! | [`SignalRcDynCell`]    | [`.as_read_only()`][aro2]                    | [`.into_read_only()`][iro4]                    | [`.downgrade()`]‌[`.into_read_only()`][iro5] | [`.into_subscription()`]‌[`.into_read_only()`][iro6]                    |
//! | [`SignalWeakDynCell`]   | [`.upgrade()`]‌[`?`]‌[`.as_read_only()`][aro2] | [`.upgrade()`]‌[`?`]‌[`.into_read_only()`][iro4] | [`.into_read_only()`][iro5]                 | [`.upgrade()`]‌[`?`]‌[`.into_subscription()`]‌[`.into_read_only()`][iro6] |
//! | [`SubscriptionDynCell`] | [`.as_read_only()`][aro2]                    | [`.unsubscribe()`]‌[`.into_read_only()`][iro4]  | [`.downgrade()`]‌[`.into_read_only()`][iro5] | [`.into_read_only()`][iro6]                                            |
//!
//! - In place of [`.as_read_only()`][aro2], you can coerce the reference.
//!
//! ## [`Effect`]
//!
//! [`Effect`] is invariant over its state and closure types and as such inconvertible.
//!
//! ## [`From`]/[`Into`]
//!
//! [`From`] and [`Into`] are available for *side effect free* conversions. These are:
//!
//! - [identity]
//! - unsizing / type-erasure
//! - upcasting (cells to read-only signals)
//! - [`ToOwned`]
//! - [`.downgrade()`] (with [`&`]‌[`Signal`] as input)
//! - **combinations of the above**
//!
//! Unsizing and upcasting conversions also available for unmanaged signal references.
//!
//! ## [`TryFrom`]/[`TryInto`]
//!
//! [`TryFrom`] and [`TryInto`] are available for *side effect free* fallible conversions. These are:
//!
//! - [`.upgrade()`] (with [`Result`]`<`[`SignalRc`]`, `[`SignalWeak`]`>` as output)
//! - **combinations of the above with unsizing / type-erasure and/or upcasting**
//!
//! [cell]: `UnmanagedSignalCell`
//! [identity]: https://doc.rust-lang.org/stable/std/convert/trait.From.html#impl-From%3CT%3E-for-T
//! [`.as_dyn_cell()`]: `Signal::as_dyn_cell`
//! [`.to_dyn_cell()`]: `Signal::to_dyn_cell`
//! [`.downgrade()`]: `Signal::downgrade`
//! [idc1]: `SignalRc::into_dyn_cell`
//! [idc2]: `SignalWeak::into_dyn_cell`
//! [idc3]: `Subscription::into_dyn_cell`
//! [`.into_subscription()`]: `SignalRc::into_subscription`
//! [`.to_subscription()`]: `Signal::to_subscription`
//! [`.unsubscribe()`]: `Subscription::unsubscribe`
//! [`.upgrade()`]: `SignalWeak::upgrade`
//! [`?`]: `core::ops::Try`
//!
//! [signal]: `UnmanagedSignal`
//! [`.as_dyn()`]: `Signal::as_dyn`
//! [`.to_dyn()`]: `Signal::to_dyn`
//! [id1]: `SignalRc::into_dyn`
//! [id2]: `SignalWeak::into_dyn`
//! [id3]: `Subscription::into_dyn`
//!
//! [aro1]: `Signal::as_read_only`
//! [tro1]: `Signal::to_read_only`
//! [iro1]: `SignalRc::into_read_only`
//! [iro2]: `SignalWeak::into_read_only`
//! [iro3]: `Subscription::into_read_only`
//!
//! [aro2]: ../struct.Signal.html#method.as_read_only-1
//! [tro2]: ../struct.Signal.html#method.to_read_only-1
//! [iro4]: ../struct.SignalRc.html#method.into_read_only-1`
//! [iro5]: ../struct.SignalWeak.html#method.into_read_only-1`
//! [iro6]: ../struct.Subscription.html#method.into_read_only-1`

#![allow(unused_imports)] // Used by documentation.

use std::{borrow::Borrow, ops::Deref};

use isoprenoid_unsend::runtime::SignalsRuntimeRef;

use crate::{
	signal_rc::SignalRcDynCell, traits::UnmanagedSignalCell, unmanaged::UnmanagedSignal, Effect,
	Signal, SignalDyn, SignalDynCell, SignalRc, SignalRcDyn, SignalWeak, SignalWeakDyn,
	SignalWeakDynCell, Subscription, SubscriptionDyn, SubscriptionDynCell,
};

/// Since 0.1.2.
impl<'a, T: ?Sized, S: Sized + UnmanagedSignal<T, SR>, SR: SignalsRuntimeRef> From<&'a S>
	for &'a dyn UnmanagedSignal<T, SR>
{
	fn from(value: &'a S) -> Self {
		value
	}
}

/// Since 0.1.2.
impl<'a, T: ?Sized, S: Sized + UnmanagedSignalCell<T, SR>, SR: SignalsRuntimeRef> From<&'a S>
	for &'a dyn UnmanagedSignalCell<T, SR>
{
	fn from(value: &'a S) -> Self {
		value
	}
}

/// Since 0.1.2.
impl<'a, T: ?Sized, SR: SignalsRuntimeRef> From<&'a dyn UnmanagedSignalCell<T, SR>>
	for &'a dyn UnmanagedSignal<T, SR>
{
	fn from(value: &'a dyn UnmanagedSignalCell<T, SR>) -> Self {
		value
	}
}

impl<
		'r,
		'a,
		T: 'a + ?Sized,
		S: 'a + Sized + UnmanagedSignal<T, SR>,
		SR: 'a + SignalsRuntimeRef,
	> From<&'r Signal<T, S, SR>> for &'r SignalDyn<'a, T, SR>
{
	fn from(value: &'r Signal<T, S, SR>) -> Self {
		value
	}
}

impl<
		'r,
		'a,
		T: 'a + ?Sized,
		S: 'a + Sized + UnmanagedSignalCell<T, SR>,
		SR: 'a + SignalsRuntimeRef,
	> From<&'r Signal<T, S, SR>> for &'r SignalDynCell<'a, T, SR>
{
	fn from(value: &'r Signal<T, S, SR>) -> Self {
		value
	}
}

/// Since 0.1.2.
impl<'r, 'a, T: 'a + ?Sized, SR: 'a + SignalsRuntimeRef> From<&'r SignalDynCell<'a, T, SR>>
	for &'r SignalDyn<'a, T, SR>
{
	fn from(value: &'r SignalDynCell<'a, T, SR>) -> Self {
		value
	}
}

impl<'a, T: 'a + ?Sized, S: 'a + Sized + UnmanagedSignal<T, SR>, SR: 'a + SignalsRuntimeRef>
	From<SignalRc<T, S, SR>> for SignalRcDyn<'a, T, SR>
{
	fn from(value: SignalRc<T, S, SR>) -> Self {
		value.into_dyn()
	}
}

impl<
		'a,
		T: 'a + ?Sized,
		S: 'a + Sized + UnmanagedSignalCell<T, SR>,
		SR: 'a + SignalsRuntimeRef,
	> From<SignalRc<T, S, SR>> for SignalRcDynCell<'a, T, SR>
{
	fn from(value: SignalRc<T, S, SR>) -> Self {
		value.into_dyn_cell()
	}
}

/// Since 0.1.2.
impl<'a, T: 'a + ?Sized, SR: 'a + SignalsRuntimeRef> From<SignalRcDynCell<'a, T, SR>>
	for SignalRcDyn<'a, T, SR>
{
	fn from(value: SignalRcDynCell<'a, T, SR>) -> Self {
		value.into_read_only()
	}
}

impl<'a, T: 'a + ?Sized, S: 'a + Sized + UnmanagedSignal<T, SR>, SR: 'a + SignalsRuntimeRef>
	From<SignalWeak<T, S, SR>> for SignalWeakDyn<'a, T, SR>
{
	fn from(value: SignalWeak<T, S, SR>) -> Self {
		value.into_dyn()
	}
}

impl<
		'a,
		T: 'a + ?Sized,
		S: 'a + Sized + UnmanagedSignalCell<T, SR>,
		SR: 'a + SignalsRuntimeRef,
	> From<SignalWeak<T, S, SR>> for SignalWeakDynCell<'a, T, SR>
{
	fn from(value: SignalWeak<T, S, SR>) -> Self {
		value.into_dyn_cell()
	}
}

/// Since 0.1.2.
impl<'a, T: 'a + ?Sized, SR: 'a + SignalsRuntimeRef> From<SignalWeakDynCell<'a, T, SR>>
	for SignalWeakDyn<'a, T, SR>
{
	fn from(value: SignalWeakDynCell<'a, T, SR>) -> Self {
		value.into_read_only()
	}
}

impl<'a, T: 'a + ?Sized, S: 'a + Sized + UnmanagedSignal<T, SR>, SR: 'a + SignalsRuntimeRef>
	From<Subscription<T, S, SR>> for SubscriptionDyn<'a, T, SR>
{
	fn from(value: Subscription<T, S, SR>) -> Self {
		value.into_dyn()
	}
}

impl<
		'a,
		T: 'a + ?Sized,
		S: 'a + Sized + UnmanagedSignalCell<T, SR>,
		SR: 'a + SignalsRuntimeRef,
	> From<Subscription<T, S, SR>> for SubscriptionDynCell<'a, T, SR>
{
	fn from(value: Subscription<T, S, SR>) -> Self {
		value.into_dyn_cell()
	}
}

/// Since 0.1.2.
impl<'a, T: 'a + ?Sized, SR: 'a + SignalsRuntimeRef> From<SubscriptionDynCell<'a, T, SR>>
	for SubscriptionDyn<'a, T, SR>
{
	fn from(value: SubscriptionDynCell<'a, T, SR>) -> Self {
		value.into_read_only()
	}
}

impl<T: ?Sized, S: Sized + UnmanagedSignal<T, SR>, SR: SignalsRuntimeRef> From<S>
	for SignalRc<T, S, SR>
{
	fn from(value: S) -> Self {
		Self::new(value)
	}
}

impl<T: ?Sized, S: ?Sized + UnmanagedSignal<T, SR>, SR: SignalsRuntimeRef> From<&Signal<T, S, SR>>
	for SignalRc<T, S, SR>
{
	fn from(value: &Signal<T, S, SR>) -> Self {
		value.to_owned()
	}
}

impl<
		'a,
		T: 'a + ?Sized,
		S: 'a + Sized + UnmanagedSignalCell<T, SR>,
		SR: 'a + SignalsRuntimeRef,
	> From<&Signal<T, S, SR>> for SignalRcDyn<'a, T, SR>
{
	fn from(value: &Signal<T, S, SR>) -> Self {
		value.to_dyn()
	}
}

impl<
		'a,
		T: 'a + ?Sized,
		S: 'a + Sized + UnmanagedSignalCell<T, SR>,
		SR: 'a + SignalsRuntimeRef,
	> From<&Signal<T, S, SR>> for SignalRcDynCell<'a, T, SR>
{
	fn from(value: &Signal<T, S, SR>) -> Self {
		value.to_dyn_cell()
	}
}

/// Since 0.1.2.
impl<'a, T: 'a + ?Sized, SR: 'a + SignalsRuntimeRef> From<&SignalDynCell<'a, T, SR>>
	for SignalRcDyn<'a, T, SR>
{
	fn from(value: &SignalDynCell<'a, T, SR>) -> Self {
		value.to_read_only()
	}
}

impl<T: ?Sized, S: ?Sized + UnmanagedSignal<T, SR>, SR: SignalsRuntimeRef> From<&Signal<T, S, SR>>
	for SignalWeak<T, S, SR>
{
	fn from(value: &Signal<T, S, SR>) -> Self {
		value.downgrade()
	}
}

impl<
		'a,
		T: 'a + ?Sized,
		S: 'a + Sized + UnmanagedSignalCell<T, SR>,
		SR: 'a + SignalsRuntimeRef,
	> From<&Signal<T, S, SR>> for SignalWeakDyn<'a, T, SR>
{
	fn from(value: &Signal<T, S, SR>) -> Self {
		value.downgrade().into_dyn()
	}
}

impl<
		'a,
		T: 'a + ?Sized,
		S: 'a + Sized + UnmanagedSignalCell<T, SR>,
		SR: 'a + SignalsRuntimeRef,
	> From<&Signal<T, S, SR>> for SignalWeakDynCell<'a, T, SR>
{
	fn from(value: &Signal<T, S, SR>) -> Self {
		value.downgrade().into_dyn_cell()
	}
}

/// Since 0.1.2.
impl<'a, T: 'a + ?Sized, SR: 'a + SignalsRuntimeRef> From<&SignalDynCell<'a, T, SR>>
	for SignalWeakDyn<'a, T, SR>
{
	fn from(value: &SignalDynCell<'a, T, SR>) -> Self {
		value.downgrade().into_read_only()
	}
}

impl<T: ?Sized, S: ?Sized + UnmanagedSignal<T, SR>, SR: SignalsRuntimeRef>
	TryFrom<SignalWeak<T, S, SR>> for SignalRc<T, S, SR>
{
	type Error = SignalWeak<T, S, SR>;

	fn try_from(value: SignalWeak<T, S, SR>) -> Result<Self, Self::Error> {
		match value.upgrade() {
			Some(strong) => Ok(strong),
			None => Err(value),
		}
	}
}

impl<
		'a,
		T: 'a + ?Sized,
		S: 'a + Sized + UnmanagedSignalCell<T, SR>,
		SR: 'a + SignalsRuntimeRef,
	> TryFrom<SignalWeak<T, S, SR>> for SignalRcDyn<'a, T, SR>
{
	type Error = SignalWeak<T, S, SR>;

	fn try_from(value: SignalWeak<T, S, SR>) -> Result<Self, Self::Error> {
		match value.upgrade() {
			Some(strong) => Ok(strong.into_dyn()),
			None => Err(value),
		}
	}
}

impl<
		'a,
		T: 'a + ?Sized,
		S: 'a + Sized + UnmanagedSignalCell<T, SR>,
		SR: 'a + SignalsRuntimeRef,
	> TryFrom<SignalWeak<T, S, SR>> for SignalRcDynCell<'a, T, SR>
{
	type Error = SignalWeak<T, S, SR>;

	fn try_from(value: SignalWeak<T, S, SR>) -> Result<Self, Self::Error> {
		match value.upgrade() {
			Some(strong) => Ok(strong.into_dyn_cell()),
			None => Err(value),
		}
	}
}

/// Since 0.1.2.
impl<'a, T: 'a + ?Sized, SR: 'a + SignalsRuntimeRef> TryFrom<SignalWeakDynCell<'a, T, SR>>
	for SignalRcDyn<'a, T, SR>
{
	type Error = SignalWeakDynCell<'a, T, SR>;

	fn try_from(value: SignalWeakDynCell<'a, T, SR>) -> Result<Self, Self::Error> {
		match value.upgrade() {
			Some(strong) => Ok(strong.into_read_only()),
			None => Err(value),
		}
	}
}
