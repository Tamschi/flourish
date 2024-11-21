use std::{
	borrow::Borrow,
	fmt::{self, Debug, Formatter},
	mem::ManuallyDrop,
	ops::Deref,
};

use isoprenoid::runtime::SignalsRuntimeRef;

use crate::{
	signal::{Signal, Strong, Weak},
	traits::{UnmanagedSignal, UnmanagedSignalCell},
	Subscription,
};

/// [`SignalArc`] after type-erasure.
pub type SignalArcDyn<'a, T, SR> = SignalArc<T, dyn 'a + UnmanagedSignal<T, SR>, SR>;

/// [`SignalArc`] after cell-type-erasure.
pub type SignalArcDynCell<'a, T, SR> = SignalArc<T, dyn 'a + UnmanagedSignalCell<T, SR>, SR>;

/// [`SignalWeak`] after type-erasure and result of [`SignalDyn::downgrade`](`crate::SignalDyn::downgrade`).
pub type SignalWeakDyn<'a, T, SR> = SignalWeak<T, dyn 'a + UnmanagedSignal<T, SR>, SR>;

/// [`SignalWeak`] after cell-type-erasure and result of [`SignalDynCell::downgrade`](`crate::SignalDynCell::downgrade`).
pub type SignalWeakDynCell<'a, T, SR> = SignalWeak<T, dyn 'a + UnmanagedSignalCell<T, SR>, SR>;

/// A weak reference to a [`Signal`].
///
/// These weak references prevent deallocation, but otherwise do allow a managed [`Signal`]
/// to be destroyed.
#[repr(transparent)]
pub struct SignalWeak<
	T: ?Sized + Send,
	S: ?Sized + UnmanagedSignal<T, SR>,
	SR: ?Sized + SignalsRuntimeRef,
> {
	pub(crate) weak: Weak<T, S, SR>,
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	SignalWeak<T, S, SR>
{
	/// Tries to obtain a [`SignalArc`] from this [`SignalWeak`].
	#[must_use]
	pub fn upgrade(&self) -> Option<SignalArc<T, S, SR>> {
		self.weak.upgrade().map(|strong| SignalArc { strong })
	}

	//TODO: Various `From` and `TryFrom` conversions, including for unsizing.

	/// Erases the (generally opaque) type parameter `S`, allowing the weak signal handle
	/// to be stored easily.
	pub fn into_dyn<'a>(self) -> SignalWeakDyn<'a, T, SR>
	where
		S: 'a + Sized,
	{
		let Self { weak } = self;
		SignalWeakDyn {
			weak: weak.into_dyn(),
		}
	}

	/// Erases the (generally opaque) type parameter `S`, allowing the weak signal cell
	/// handle to be stored easily.
	pub fn into_dyn_cell<'a>(self) -> SignalWeakDynCell<'a, T, SR>
	where
		S: 'a + Sized + UnmanagedSignalCell<T, SR>,
	{
		let Self { weak } = self;
		SignalWeakDynCell {
			weak: weak.into_dyn_cell(),
		}
	}
}

/// A reference-counting [`Signal`] handle that is all of [`Clone`], [`Send`], [`Sync`] and [`Unpin`].
///
/// Inherits value accessors from [`Signal`].
///
/// Note that [`Signal`] implements [`ToOwned<Owned = SignalArc>`](`ToOwned`),
/// so in cases where ownership is not always required, prefer [`&Signal`](`&`) as function parameter type!
#[must_use = "Signals are generally inert unless subscribed to."]
pub struct SignalArc<
	T: ?Sized + Send,
	S: ?Sized + UnmanagedSignal<T, SR>,
	SR: ?Sized + SignalsRuntimeRef,
> {
	pub(super) strong: Strong<T, S, SR>,
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef> Clone
	for SignalArc<T, S, SR>
{
	fn clone(&self) -> Self {
		Self {
			strong: self.strong.clone(),
		}
	}
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef> Clone
	for SignalWeak<T, S, SR>
{
	fn clone(&self) -> Self {
		Self {
			weak: self.weak.clone(),
		}
	}
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef> Debug
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

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef> Deref
	for SignalArc<T, S, SR>
{
	type Target = Signal<T, S, SR>;

	fn deref(&self) -> &Self::Target {
		&self.strong
	}
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Borrow<Signal<T, S, SR>> for SignalArc<T, S, SR>
{
	fn borrow(&self) -> &Signal<T, S, SR> {
		self.strong.borrow()
	}
}

unsafe impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Send for SignalArc<T, S, SR>
{
}
unsafe impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	Sync for SignalArc<T, S, SR>
{
}

impl<T: ?Sized + Send, S: ?Sized + UnmanagedSignal<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	SignalArc<T, S, SR>
{
	/// Creates a new [`SignalArc`] from the provided [`UnmanagedSignal`].
	///
	/// For additional constructors, see [`Signal`].
	pub fn new(unmanaged: S) -> Self
	where
		S: Sized,
	{
		SignalArc {
			strong: Strong::pin(unmanaged),
		}
	}

	//TODO: Various `From` and `TryFrom` conversions, including for unsizing.

	/// Erases the (generally opaque) type parameter `S`, allowing the signal handle to
	/// be stored easily.
	pub fn into_dyn<'a>(self) -> SignalArcDyn<'a, T, SR>
	where
		S: 'a + Sized,
	{
		let Self { strong } = self;
		SignalArcDyn {
			strong: strong.into_dyn(),
		}
	}

	/// Erases the (generally opaque) type parameter `S`, allowing the signal cell handle
	/// to be stored easily.
	pub fn into_dyn_cell<'a>(self) -> SignalArcDynCell<'a, T, SR>
	where
		S: 'a + Sized + UnmanagedSignalCell<T, SR>,
	{
		let Self { strong } = self;
		SignalArcDynCell {
			strong: strong.into_dyn_cell(),
		}
	}

	/// Subscribes to the managed [`Signal`], converting this [`SignalArc`] into a [`Subscription`].
	///
	/// Compared to [`Signal::to_subscription`], this avoids some memory barriers.
	pub fn into_subscription(self) -> Subscription<T, S, SR> {
		self.strong._managed().subscribe();
		Subscription {
			subscribed: ManuallyDrop::new(self.strong),
		}
	}
}

impl<T: ?Sized + Send, S: Sized + UnmanagedSignalCell<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	SignalArc<T, S, SR>
{
	/// Obscures the cell API, allowing only reads and subscriptions.
	pub fn into_read_only<'a>(self) -> SignalArc<T, impl 'a + UnmanagedSignal<T, SR>, SR>
	where
		S: 'a,
	{
		//FIXME: This is *probably* inefficient.
		self.as_read_only().to_owned()
	}

	/// Equivalent to a getter/setter splitter.
	pub fn into_read_only_and_self<'a>(
		self,
	) -> (SignalArc<T, impl 'a + UnmanagedSignal<T, SR>, SR>, Self)
	where
		S: 'a,
	{
		(self.as_read_only().to_owned(), self)
	}

	/// A getter/setter splitter like [`into_read_only_and_self`](`SignalArc::into_read_only_and_self`),
	/// but additionally type-erases the type parameter `S` for easy storage.
	pub fn into_dyn_read_only_and_self<'a>(
		self,
	) -> (SignalArcDyn<'a, T, SR>, SignalArcDynCell<'a, T, SR>)
	where
		S: 'a,
	{
		(self.as_dyn().to_owned(), self.into_dyn_cell())
	}
}

impl<T: ?Sized + Send, S: Sized + UnmanagedSignalCell<T, SR>, SR: ?Sized + SignalsRuntimeRef>
	SignalWeak<T, S, SR>
{
	/// Obscures the cell API, allowing only reads and subscriptions.
	pub fn into_read_only<'a>(self) -> SignalWeak<T, impl 'a + UnmanagedSignal<T, SR>, SR>
	where
		S: 'a,
	{
		unsafe {
			//SAFETY: Prevents dropping of the original `Weak`,
			//        so that the net count doesn't change.
			let this = ManuallyDrop::new(self);
			SignalWeak {
				weak: this.weak.unsafe_copy(),
			}
		}
	}

	/// Equivalent to a getter/setter splitter.
	pub fn into_read_only_and_self<'a>(
		self,
	) -> (SignalWeak<T, impl 'a + UnmanagedSignal<T, SR>, SR>, Self)
	where
		S: 'a,
	{
		(self.clone().into_read_only(), self)
	}

	/// A getter/setter splitter like [`.into_read_only_and_self()`](`SignalWeak::into_read_only_and_self`),
	/// but additionally type-erases the type parameter `S` for easy storage.
	pub fn into_dyn_read_only_and_self<'a>(
		self,
	) -> (SignalWeakDyn<'a, T, SR>, SignalWeakDynCell<'a, T, SR>)
	where
		S: 'a,
	{
		(self.clone().into_dyn(), self.into_dyn_cell())
	}
}
