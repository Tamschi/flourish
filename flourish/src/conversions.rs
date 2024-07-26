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

use std::{marker::PhantomData, mem, pin::Pin, sync::Arc};

use isoprenoid::runtime::SignalsRuntimeRef;

use crate::{
	raw::{SourceCell, Subscribable},
	signal_cell::SignalCellRef,
	SignalCellDyn, SignalCellRefDyn, SignalCellSR, SignalDyn, SignalSR,
};

// into `SignalCellSR` / into `SignalCellDyn`

impl<T: ?Sized + Send, S: ?Sized + SourceCell<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	From<SignalCellRef<'_, T, S, SR>> for SignalCellSR<T, S, SR>
{
	fn from(value: SignalCellRef<T, S, SR>) -> Self {
		Self {
			source_cell: unsafe {
				Arc::increment_strong_count(value.source_cell);
				Pin::new_unchecked(Arc::from_raw(value.source_cell))
			},
			upcast: value.upcast,
		}
	}
}

// into `SignalCellDyn`

impl<
		'a,
		T: 'a + ?Sized + Send,
		S: 'a + Sized + SourceCell<T, SR>,
		SR: 'a + ?Sized + SignalsRuntimeRef,
	> From<SignalCellSR<T, S, SR>> for SignalCellDyn<'a, T, SR>
{
	fn from(value: SignalCellSR<T, S, SR>) -> Self {
		value.into_dyn()
	}
}

impl<
		'a,
		T: 'a + ?Sized + Send,
		S: 'a + Sized + SourceCell<T, SR>,
		SR: 'a + ?Sized + SignalsRuntimeRef,
	> From<SignalCellRef<'_, T, S, SR>> for SignalCellDyn<'a, T, SR>
{
	fn from(value: SignalCellRef<'_, T, S, SR>) -> Self {
		let value: SignalCellSR<T, S, SR> = value.into();
		value.into()
	}
}

// into `SignalCellRef` / into `SignalCellRefDyn`

impl<'r, T: ?Sized + Send, S: ?Sized + SourceCell<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	From<&'r SignalCellSR<T, S, SR>> for SignalCellRef<'r, T, S, SR>
{
	fn from(value: &'r SignalCellSR<T, S, SR>) -> Self {
		Self {
			source_cell: unsafe {
				let ptr = Arc::into_raw(Pin::into_inner_unchecked(Pin::clone(&value.source_cell)));
				Arc::decrement_strong_count(ptr);
				ptr
			},
			upcast: value.upcast,
			_phantom: PhantomData,
		}
	}
}

// into `SignalCellRefDyn`

impl<
		'r,
		'a,
		T: 'a + ?Sized + Send,
		S: 'a + Sized + SourceCell<T, SR>,
		SR: 'a + ?Sized + SignalsRuntimeRef,
	> From<&'r SignalCellSR<T, S, SR>> for SignalCellRefDyn<'r, 'a, T, SR>
{
	fn from(value: &'r SignalCellSR<T, S, SR>) -> Self {
		Self {
			source_cell: unsafe {
				let ptr = Arc::into_raw(Pin::into_inner_unchecked(Pin::clone(&value.source_cell)));
				Arc::decrement_strong_count(ptr);
				ptr
			},
			upcast: value.upcast,
			_phantom: PhantomData,
		}
	}
}

impl<
		'r,
		'a,
		T: 'a + ?Sized + Send,
		S: 'a + Sized + SourceCell<T, SR>,
		SR: 'a + ?Sized + SignalsRuntimeRef,
	> From<&'r SignalCellRef<'r, T, S, SR>> for SignalCellRefDyn<'r, 'a, T, SR>
{
	fn from(value: &'r SignalCellRef<'r, T, S, SR>) -> Self {
		Self {
			source_cell: value.source_cell,
			upcast: value.upcast,
			_phantom: PhantomData,
		}
	}
}

// TODO

impl<
		'a,
		T: 'a + ?Sized + Send,
		S: 'a + Sized + Subscribable<T, SR>,
		SR: 'a + ?Sized + SignalsRuntimeRef,
	> From<SignalSR<T, S, SR>> for SignalDyn<'a, T, SR>
{
	fn from(value: SignalSR<T, S, SR>) -> Self {
		let SignalSR { source, _phantom } = value;
		Self { source, _phantom }
	}
}

impl<
		'a,
		T: 'a + ?Sized + Send,
		S: 'a + Sized + SourceCell<T, SR>,
		SR: 'a + ?Sized + SignalsRuntimeRef,
	> From<SignalCellSR<T, S, SR>> for SignalDyn<'a, T, SR>
{
	fn from(value: SignalCellSR<T, S, SR>) -> Self {
		value.into_dyn().into()
	}
}

impl<'a, T: 'a + ?Sized + Send, SR: 'a + ?Sized + SignalsRuntimeRef> From<SignalCellDyn<'a, T, SR>>
	for SignalDyn<'a, T, SR>
{
	fn from(value: SignalCellDyn<'a, T, SR>) -> Self {
		let SignalCellDyn {
			source_cell,
			upcast,
		} = value;
		Self {
			source: unsafe {
				mem::forget(source_cell);
				Pin::new_unchecked(Arc::from_raw(upcast.0))
			},
			_phantom: PhantomData,
		}
	}
}

//TODO: Conversion from raw SourceCell.
