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
//! `↖` indicates that the conversion is part of a superset conversion, one cell diagonally to the upper left.  
//! Entries prefixed with '(`&`)' convert from references or borrow instead of consuming the handle.
//!
//! **Macro authors should use qualified [`From`] and [`Into`] conversions instead of duck-typing the static-dispatch API.**
//!
//! Note that only side-effect-free conversions are supported via [`From`]:
//!
//! | from ↓ \ into →         | [`ArcSignalCell`] | [`ArcSignalCellDyn`]   | [`&SignalCell`][SC]              | [`&SignalCellDyn`][SCD] |
//! |-------------------------|-------------------|------------------------|----------------------------------|-------------------------|
//! | [`ArcSignalCell`]       | [identity]        | [`.into_dyn()`][ASCid] | [`Deref`], [`Borrow`], [`AsRef`] | [`.as_dyn()`][ASCad]    |
//! | [`ArcSignalCellDyn`]    | →                 | [identity]             | →                                | ↖                       |
//! | [`&SignalCell`][SC]     | [`ToOwned`]       | [`.to_dyn()`][SCtd]    | [identity]                       | [`.as_dyn()`][SCad]     |
//! | [`&SignalCellDyn`][SCD] | →                 | ↖                      | →                                | [identity]              |
//!
//! | from ↓ \ into →         | [`ArcSignal`]                                           | [`ArcSignalDyn`]                                                    | [`&Signal`][S]                      | [`&SignalDyn`][SD]                      |
//! |-------------------------|--------------------------------------------------------|------------------------------------------------------------------|------------------------------------|---------------------------------------|
//! | [`ArcSignalCell`]       | [`.into_signal()`][is1]<br>(`&`)&nbsp;[`.to_signal()`][ts1] | [`.into_signal_dyn()`][isd1]<br>(`&`)&nbsp;[`.to_signal_dyn()`][tsd1] | (`&`)&nbsp;[`.as_signal_ref()`][asr1]   | (`&`)&nbsp;[`.as_signal_ref_dyn()`][asrd1] |
//! | [`ArcSignalCellDyn`]    | →                                                      | [`.into_signal()`][is1]<br>(`&`)&nbsp;[`.to_signal()`][ts1]           | →                                  | (`&`)&nbsp;[`.as_signal_ref()`][asr1]      |
//! | [`&SignalCell`][SC]     | (`&`)&nbsp;[`.to_signal()`][ts2]                            | (`&`)&nbsp;[`.to_signal_dyn()`][tsd2]                                 | [`.into_signal_ref()`][isr1]       | [`.into_signal_ref()`][isrd1]         |
//! | [`&SignalCellDyn`][SCD] | →                                                      | (`&`)&nbsp;[`.to_signal()`][ts2]                                      | →                                  | [`.into_signal_ref()`][isr1]          |
//! | [`ArcSignal`]           | [identity]                                             | [`.into_dyn()`][ASid]                                             | (`&`)&nbsp;[`.as_ref()`][ar2]           | (`&`)&nbsp;[`.as_ref_dyn()`][ard2]         |
//! | [`ArcSignalDyn`]        | →                                                      | [identity]                                                       | →                                  | (`&`)&nbsp;[`.as_ref()`][ar2]              |
//! | [`&Signal`][S]          | (`&`)&nbsp;[`.clone()`][c2]                                 | (`&`)&nbsp;[`.clone_dyn()`][cd2]                                      | [identity]                         | [`.into_dyn()`][Sid]                  |
//! | [`&SignalDyn`][SD]      | →                                                      | (`&`)&nbsp;[`.clone()`][c2]                                           | →                                  | [identity]                            |
//!
//! //TODO: Formatting!
//! //TODO: Table for subscriptions.
//! //TODO: Note that `Effects` aren't convertible.
//!
//! //TODO: On second thought, remove most of the convenience methods, implement [`Borrow`], [`ToOwned`], [`Deref`] and possibly [`AsRef`] instead.
//! //      (Refcounting handles can wrap Refs!)
//!
//! [SC]: `SignalCell`
//! [SCD]: `SignalCellDyn`
//! [S]: `Signal`
//! [SD]: `SignalDyn`
//!
//! [identity]: https://doc.rust-lang.org/stable/std/convert/trait.From.html#impl-From%3CT%3E-for-T
//! [ASCid]: `ArcSignalCell::into_dyn`
//! [SCtd]: `SignalCell::to_dyn`
//! [ASCad]: `ArcSignalCell::as_dyn`
//! [SCad]: `SignalCell::into_dyn`
//! [ASid]: `ArcSignal::into_dyn`
//! [Sid]: `Signal::into_dyn`
//!
//! Special cases like [`Signal`](`crate::Signal`) of [`ArcSignal`] are omitted for clarity.
//!
//! Entries that say '`.into_dyn()`' should be upgradable to unsizing coercions eventually.
//!
//! Each [`ArcSourcePin`] above has an associated [`WeakSourcePin`] with equivalent conversions:
//! Types can be converted among themselves just like their strong variant, but up- and downgrades
//! must be explicit.
//!
//! ## Side-effect conversions

use std::{
	borrow::Borrow,
	marker::PhantomData,
	mem::{self, ManuallyDrop},
	ops::Deref,
	pin::Pin,
	sync::{Arc, Weak},
};

use isoprenoid::runtime::SignalsRuntimeRef;

use crate::{
	signal_cell::SignalCell,
	unmanaged::{Subscribable, UnmanagedSignalCell},
	ArcSignal, ArcSignalCell, ArcSignalCellDyn, ArcSignalDyn, SignalCellDyn,
};

impl<T: ?Sized + Send, S: Sized + UnmanagedSignalCell<T, SR>, SR: SignalsRuntimeRef>
	ArcSignalCell<T, S, SR>
{
	pub fn into_dyn<'a>(self) -> ArcSignalCellDyn<'a, T, SR>
	where
		S: 'a + Sized,
	{
		ArcSignalCellDyn {
			arc: self.arc.weak_dyn.upgrade().expect("cyclic"),
		}
	}
}

impl<T: ?Sized + Send, S: Sized + UnmanagedSignalCell<T, SR>, SR: SignalsRuntimeRef>
	SignalCell<T, S, SR>
{
	pub fn as_dyn<'a>(&self) -> &SignalCellDyn<'a, T, SR>
	where
		S: 'a + Sized,
	{
		unsafe {
			let this = Weak::clone(&self.weak_dyn);
			drop((&this as *const Weak<SignalCellDyn<'static, T, SR>>).read());
			&*Weak::into_raw(this)
		}
	}

	pub fn to_dyn<'a>(&self) -> ArcSignalCellDyn<'a, T, SR>
	where
		S: 'a + Sized,
	{
		self.to_owned().into_dyn()
	}
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignalCell<T, SR>, SR: ?Sized + SignalsRuntimeRef> Deref
	for ArcSignalCell<T, S, SR>
{
	type Target = SignalCell<T, S, SR>;

	fn deref(&self) -> &Self::Target {
		self.arc.deref()
	}
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignalCell<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Borrow<SignalCell<T, S, SR>> for ArcSignalCell<T, S, SR>
{
	fn borrow(&self) -> &SignalCell<T, S, SR> {
		self.arc.borrow()
	}
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignalCell<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	AsRef<SignalCell<T, S, SR>> for ArcSignalCell<T, S, SR>
{
	fn as_ref(&self) -> &SignalCell<T, S, SR> {
		self.arc.as_ref()
	}
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignalCell<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	ToOwned for SignalCell<T, S, SR>
{
	type Owned = ArcSignalCell<T, S, SR>;

	fn to_owned(&self) -> ArcSignalCell<T, S, SR> {
		ArcSignalCell {
			arc: self.weak.upgrade().expect("cyclic"),
		}
	}
}

//TODO: Conversion from raw UnmanagedSignalCell.
