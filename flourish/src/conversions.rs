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
//! | from ↓ \ into →      | [`SignalCellSR`]       | [`SignalCellDyn`]           | [`SignalCellRef`]        | [`SignalCellRefDyn`]          |
//! |----------------------|------------------------|-----------------------------|--------------------------|-------------------------------|
//! | [`SignalCellSR`]     | [identity]             | [`.into_dyn()`][id1]        | (`&`)&nbsp;[`.as_ref()`][ar1] | (`&`)&nbsp;[`.as_ref_dyn()`][ard1] |
//! | [`SignalCellDyn`]    | →                      | [identity]                  | →                        | (`&`)&nbsp;[`.as_ref()`][ar1]      |
//! | [`SignalCellRef`]    | (`&`)&nbsp;[`.clone()`][c1] | (`&`)&nbsp;[`.clone_dyn()`][cd1] | [identity]               | [`.into_dyn()`][id2]          |
//! | [`SignalCellRefDyn`] | →                      | (`&`)&nbsp;[`.clone()`][c1]      | →                        | [identity]                    |
//!
//! | from ↓ \ into →      | [`SignalSR`]                                           | [`SignalDyn`]                                                    | [`SignalRef`]                      | [`SignalRefDyn`]                      |
//! |----------------------|--------------------------------------------------------|------------------------------------------------------------------|------------------------------------|---------------------------------------|
//! | [`SignalCellSR`]     | [`.into_signal()`][is1]<br>(`&`)&nbsp;[`.to_signal()`][ts1] | [`.into_signal_dyn()`][isd1]<br>(`&`)&nbsp;[`.to_signal_dyn()`][tsd1] | (`&`)&nbsp;[`.as_signal_ref()`][asr1]   | (`&`)&nbsp;[`.as_signal_ref_dyn()`][asrd1] |
//! | [`SignalCellDyn`]    | →                                                      | [`.into_signal()`][is1]<br>(`&`)&nbsp;[`.to_signal()`][ts1]           | →                                  | (`&`)&nbsp;[`.as_signal_ref()`][asr1]      |
//! | [`SignalCellRef`]    | (`&`)&nbsp;[`.to_signal()`][ts2]                            | (`&`)&nbsp;[`.to_signal_dyn()`][tsd2]                                 | [`.into_signal_ref()`][isr1]       | [`.into_signal_ref()`][isrd1]         |
//! | [`SignalCellRefDyn`] | →                                                      | (`&`)&nbsp;[`.to_signal()`][ts2]                                      | →                                  | [`.into_signal_ref()`][isr1]          |
//! | [`SignalSR`]         | [identity]                                             | [`.into_dyn()`][id3]                                             | (`&`)&nbsp;[`.as_ref()`][ar2]           | (`&`)&nbsp;[`.as_ref_dyn()`][ard2]         |
//! | [`SignalDyn`]        | →                                                      | [identity]                                                       | →                                  | (`&`)&nbsp;[`.as_ref()`][ar2]              |
//! | [`SignalRef`]        | (`&`)&nbsp;[`.clone()`][c2]                                 | (`&`)&nbsp;[`.clone_dyn()`][cd2]                                      | [identity]                         | [`.into_dyn()`][id4]                  |
//! | [`SignalRefDyn`]     | →                                                      | (`&`)&nbsp;[`.clone()`][c2]                                           | →                                  | [identity]                            |
//!
//! //TODO: Formatting!
//! //TODO: Table for subscriptions.
//! //TODO: Note that `Effects` aren't convertible.
//!
//! //TODO: On second thought, remove most of the convenience methods, implement [`Borrow`], [`ToOwned`], [`Deref`] and possibly [`AsRef`] instead.
//! //      (Refcounting handles can wrap Refs!)
//!
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

use isoprenoid::runtime::SignalsRuntimeRef;

use crate::{
	signal_arc::SignalArcDynCell, traits::UnmanagedSignalCell, unmanaged::UnmanagedSignal,
	SignalArc, SignalArcDyn,
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
