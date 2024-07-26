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
//! Entries prefixed with '`*`' unnecessarily convert to a reference and should be dereferenced immediately afterwards.
//!
//! **Macro authors should use qualified [`From`] and [`Into`] conversions instead of duck-typing the static-dispatch API.**
//!
//! Note that only side-effect-free conversions are supported via [`From`]:
//!
//! | from ↓ \ into →      | [`SignalCellSR`]       | [`SignalCellDyn`]           | [`SignalCellRef`]           | [`SignalCellRefDyn`]        |
//! |----------------------|------------------------|-----------------------------|-----------------------------|-----------------------------|
//! | [`SignalCellSR`]     | [identity]             | [`.into_dyn()`][id1]        | TODO | `.as_ref().into_dyn()`      |
//! | [`SignalCellDyn`]    | →                      | [identity]                  | →                           | TODO |
//! | [`SignalCellRef`]    | [`.into_owned()`][io1] | [`.into_owned_dyn()`][iod1] | [identity]                  | [`.into_dyn()`][id2]        |
//! | [`SignalCellRefDyn`] | →                      | [`.into_owned()`][io1]      | →                           | [identity]                  |
//!
//! | from ↓ \ into →      | [`SignalSR`]                                           | [`SignalDyn`]                                                    | [`SignalRef`]                      | [`SignalRefDyn`]                      |
//! |----------------------|--------------------------------------------------------|------------------------------------------------------------------|------------------------------------|---------------------------------------|
//! | [`SignalCellSR`]     |  |  |    |  |
//! | [`SignalCellDyn`]    | →         |            | →    |       |
//! | [`SignalCellRef`]    |   |    |        |          |
//! | [`SignalCellRefDyn`] | →         | | →    ||
//! | [`SignalSR`]         |         [identity]       |   | |          |
//! | [`SignalDyn`]        | →         |   [identity]        | →    ||
//! | [`SignalRef`]        |    |        | [identity]          |    |
//! | [`SignalRefDyn`]     | →         | | →    | [identity]  |
//!
//! //TODO: Formatting!
//! //TODO: Table for subscriptions.
//! //TODO: Note that `Effects` aren't convertible.
//!
//! //TODO: On second thought, remove most of the convenience methods, implement [`Borrow`], [`ToOwned`], [`Deref`] and possibly [`AsRef`] instead.
//! //      (Refcounting handles can wrap Refs!)
//!
//! [identity]: https://doc.rust-lang.org/stable/std/convert/trait.From.html#impl-From%3CT%3E-for-T
//!
//! [io1]: `SignalCellRef::into_owned`
//! [id1]: `SignalCellSR::into_dyn`
//! [iod1]: `SignalCellRef::into_owned_dyn`
//! [id2]: `SignalCellRef::into_dyn`
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

//TODO: Make inherent subscriptions non-unique in oder to have a nicer API for e.g. resource caches!

use std::{borrow::Borrow, marker::PhantomData, mem, pin::Pin, sync::Arc};

use isoprenoid::runtime::SignalsRuntimeRef;

use crate::{
	raw::{SourceCell, Subscribable},
	signal_cell::SignalCellRef,
	SignalCellDyn, SignalCellRefDyn, SignalCellSR, SignalDyn, SignalSR,
};

impl<T: ?Sized + Send, S: ?Sized + SourceCell<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	From<SignalCellRef<'_, T, S, SR>> for SignalCellSR<T, S, SR>
{
	fn from(value: SignalCellRef<T, S, SR>) -> Self {
		value.into_owned()
	}
}

impl<T: ?Sized + Send, S: ?Sized + SourceCell<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	SignalCellRef<'_, T, S, SR>
{
	pub fn into_owned(self) -> SignalCellSR<T, S, SR> {
		SignalCellSR {
			arc: SignalCellRef {
				source_cell: unsafe {
					Arc::increment_strong_count(self.source_cell);
					self.source_cell
				},
				upcast: self.upcast,
				_phantom: PhantomData,
			},
		}
	}

	pub fn into_owned_dyn<'a>(self) -> SignalCellDyn<'a, T, SR>
	where
		T: 'a,
		S: 'a + Sized,
		SR: 'a,
	{
		self.into_owned().into_dyn()
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

impl<T: ?Sized + Send, S: ?Sized + SourceCell<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	SignalCellSR<T, S, SR>
{
	pub fn into_dyn<'a>(self) -> SignalCellDyn<'a, T, SR>
	where
		T: 'a,
		S: 'a + Sized,
		SR: 'a,
	{
		SignalCellDyn {
			arc: self.arc.into_dyn(),
		}
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
		value.into_owned_dyn()
	}
}

impl<'r, T: ?Sized + Send, S: ?Sized + SourceCell<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	From<&'r SignalCellSR<T, S, SR>> for SignalCellRef<'r, T, S, SR>
{
	fn from(value: &'r SignalCellSR<T, S, SR>) -> Self {
		*value.borrow()
	}
}

impl<
		'r,
		'a,
		T: 'a + ?Sized + Send,
		S: 'a + Sized + SourceCell<T, SR>,
		SR: 'a + ?Sized + SignalsRuntimeRef,
	> From<SignalCellRef<'r, T, S, SR>> for SignalCellRefDyn<'r, 'a, T, SR>
{
	fn from(value: SignalCellRef<'r, T, S, SR>) -> Self {
		value.into_dyn()
	}
}

impl<'r, T: ?Sized + Send, S: ?Sized + SourceCell<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	SignalCellRef<'r, T, S, SR>
{
	pub fn into_dyn<'a>(self) -> SignalCellRefDyn<'r, 'a, T, SR>
	where
		T: 'a,
		S: 'a + Sized,
		SR: 'a,
	{
		SignalCellRefDyn {
			source_cell: self.source_cell,
			upcast: self.upcast,
			_phantom: PhantomData,
		}
	}
}

// TODO

//TODO: Conversion from raw SourceCell.
