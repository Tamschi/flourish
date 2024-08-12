use std::{
	borrow::Borrow,
	fmt::{self, Debug, Formatter},
	ops::Deref,
};

use isoprenoid::runtime::SignalsRuntimeRef;

use crate::{
	signal::{Signal, Strong, Weak},
	traits::{Subscribable, UnmanagedSignalCell},
};

/// Type of [`SignalSR`]s after type-erasure. Dynamic dispatch.
pub type SignalArcDyn<'a, T, SR> = SignalArc<T, dyn 'a + Subscribable<T, SR>, SR>;

/// Type of [`SignalSR`]s after cell-type-erasure. Dynamic dispatch.
pub type SignalArcDynCell<'a, T, SR> = SignalArc<T, dyn 'a + UnmanagedSignalCell<T, SR>, SR>;

/// Type of [`SignalWeak`]s after type-erasure or [`SignalDyn`] after downgrade. Dynamic dispatch.
pub type SignalWeakDyn<'a, T, SR> = SignalWeak<T, dyn 'a + Subscribable<T, SR>, SR>;

/// Type of [`SignalWeak`]s after cell-type-erasure or [`SignalDynCell`] after downgrade. Dynamic dispatch.
pub type SignalWeakDynCell<'a, T, SR> = SignalWeak<T, dyn 'a + UnmanagedSignalCell<T, SR>, SR>;

#[repr(transparent)]
pub struct SignalWeak<
	T: ?Sized + Send,
	S: ?Sized + Subscribable<T, SR>,
	SR: ?Sized + SignalsRuntimeRef,
> {
	pub(crate) weak: Weak<T, S, SR>,
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	SignalWeak<T, S, SR>
{
	#[must_use]
	pub fn upgrade(&self) -> Option<SignalArc<T, S, SR>> {
		self.weak.upgrade().map(|strong| SignalArc { strong })
	}
}

/// A largely type-erased signal handle that is all of [`Clone`], [`Send`], [`Sync`] and [`Unpin`].
///
/// To access values, import [`SourcePin`].
///
/// Signals are not evaluated unless they are subscribed-to (or on demand if if not current).  
/// Uncached signals are instead evaluated on direct demand **only** (but still communicate subscriptions and invalidation).
#[must_use = "Signals are generally inert unless subscribed to."]
pub struct SignalArc<
	T: ?Sized + Send,
	S: ?Sized + Subscribable<T, SR>,
	SR: ?Sized + SignalsRuntimeRef,
> {
	pub(super) strong: Strong<T, S, SR>,
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Clone
	for SignalArc<T, S, SR>
{
	fn clone(&self) -> Self {
		Self {
			strong: self.strong.clone(),
		}
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Clone
	for SignalWeak<T, S, SR>
{
	fn clone(&self) -> Self {
		Self {
			weak: self.weak.clone(),
		}
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Debug
	for SignalArc<T, S, SR>
where
	T: Debug,
{
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		self.strong.clone_runtime_ref().run_detached(|| {
			f.debug_struct("SignalSR")
				.field("(value)", &&**self.strong.read_exclusive_dyn())
				.finish_non_exhaustive()
		})
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Deref
	for SignalArc<T, S, SR>
{
	type Target = Signal<T, S, SR>;

	fn deref(&self) -> &Self::Target {
		&self.strong
	}
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Borrow<Signal<T, S, SR>> for SignalArc<T, S, SR>
{
	fn borrow(&self) -> &Signal<T, S, SR> {
		self.strong.borrow()
	}
}

unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Send
	for SignalArc<T, S, SR>
{
}
unsafe impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef> Sync
	for SignalArc<T, S, SR>
{
}

impl<T: ?Sized + Send, S: ?Sized + Subscribable<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	SignalArc<T, S, SR>
{
	/// Creates a new [`SignalSR`] from the provided raw [`Subscribable`].
	pub fn new(source: S) -> Self
	where
		S: Sized,
	{
		SignalArc {
			strong: Strong::pin(source),
		}
	}

	//TODO: Various `From` and `TryFrom` conversions, including for unsizing.

	pub fn into_dyn<'a>(self) -> SignalArcDyn<'a, T, SR>
	where
		S: 'a + Sized,
	{
		let Self { strong } = self;
		SignalArcDyn {
			strong: strong.into_dyn(),
		}
	}

	pub fn into_dyn_cell<'a>(self) -> SignalArcDynCell<'a, T, SR>
	where
		S: 'a + Sized + UnmanagedSignalCell<T, SR>,
	{
		let Self { strong } = self;
		SignalArcDynCell {
			strong: strong.into_dyn_cell(),
		}
	}
}

impl<T: ?Sized + Send, S: Sized + UnmanagedSignalCell<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	SignalArc<T, S, SR>
{
	pub fn into_read_only<'a>(self) -> SignalArc<T, impl 'a + Subscribable<T, SR>, SR>
	where
		S: 'a,
	{
		//FIXME: This is *probably* inefficient.
		self.as_read_only().to_owned()
	}

	pub fn into_read_only_and_self<'a>(
		self,
	) -> (SignalArc<T, impl 'a + Subscribable<T, SR>, SR>, Self)
	where
		S: 'a,
	{
		(self.as_read_only().to_owned(), self)
	}

	pub fn into_read_only_and_self_dyn<'a>(
		self,
	) -> (SignalArcDyn<'a, T, SR>, SignalArcDynCell<'a, T, SR>)
	where
		S: 'a,
	{
		(self.as_dyn().to_owned(), self.into_dyn_cell())
	}
}

/// Duplicated to avoid identities.
mod private {
	use std::{borrow::Borrow, ops::Deref};

	use crate::traits::Guard;

	pub struct BoxedGuardDyn<'r, T: ?Sized>(pub(super) Box<dyn 'r + Guard<T>>);

	impl<T: ?Sized> Guard<T> for BoxedGuardDyn<'_, T> {}

	impl<T: ?Sized> Deref for BoxedGuardDyn<'_, T> {
		type Target = T;

		fn deref(&self) -> &Self::Target {
			self.0.deref()
		}
	}

	impl<T: ?Sized> Borrow<T> for BoxedGuardDyn<'_, T> {
		fn borrow(&self) -> &T {
			(*self.0).borrow()
		}
	}
}
