use std::{
	borrow::Borrow,
	fmt::{self, Debug, Formatter},
	mem::ManuallyDrop,
	ops::Deref,
};

use isoprenoid_unsend::runtime::SignalsRuntimeRef;

use crate::{
	signal::{Signal, Strong, Weak},
	traits::{UnmanagedSignal, UnmanagedSignalCell},
	Subscription,
};

/// [`SignalRc`] after type-erasure.
pub type SignalRcDyn<'a, T, SR> = SignalRc<T, dyn 'a + UnmanagedSignal<T, SR>, SR>;

/// [`SignalRc`] after cell-type-erasure.
pub type SignalRcDynCell<'a, T, SR> = SignalRc<T, dyn 'a + UnmanagedSignalCell<T, SR>, SR>;

/// [`SignalWeak`] after type-erasure and result of [`SignalDyn::downgrade`](`crate::SignalDyn::downgrade`).
pub type SignalWeakDyn<'a, T, SR> = SignalWeak<T, dyn 'a + UnmanagedSignal<T, SR>, SR>;

/// [`SignalWeak`] after cell-type-erasure and result of [`SignalDynCell::downgrade`](`crate::SignalDynCell::downgrade`).
pub type SignalWeakDynCell<'a, T, SR> = SignalWeak<T, dyn 'a + UnmanagedSignalCell<T, SR>, SR>;

/// A weak reference to a [`Signal`].
///
/// These weak references prevent deallocation, but otherwise do allow a managed [`Signal`]
/// to be destroyed.
#[repr(transparent)]
pub struct SignalWeak<T: ?Sized, S: ?Sized + UnmanagedSignal<T, SR>, SR: SignalsRuntimeRef> {
	pub(crate) weak: Weak<T, S, SR>,
}

impl<T: ?Sized, S: ?Sized + UnmanagedSignal<T, SR>, SR: SignalsRuntimeRef> SignalWeak<T, S, SR> {
	/// Tries to obtain a [`SignalRc`] from this [`SignalWeak`].
	#[must_use]
	pub fn upgrade(&self) -> Option<SignalRc<T, S, SR>> {
		self.weak.upgrade().map(|strong| SignalRc { strong })
	}

	/// Erases the (generally opaque) type parameter `S`, allowing the weak signal handle
	/// to be stored easily.
	#[must_use]
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
	#[must_use]
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

/// A reference-counting [`Signal`] handle that is [`Clone`] and [`Unpin`].
///
/// Inherits value accessors from [`Signal`].
///
/// Note that [`Signal`] implements [`ToOwned<Owned = SignalRc>`](`ToOwned`),
/// so in cases where ownership is not always required, prefer [`&Signal`](`&`) as function parameter type!
#[must_use = "Signals are generally inert unless subscribed to."]
pub struct SignalRc<T: ?Sized, S: ?Sized + UnmanagedSignal<T, SR>, SR: SignalsRuntimeRef> {
	pub(super) strong: Strong<T, S, SR>,
}

impl<T: ?Sized, S: ?Sized + UnmanagedSignal<T, SR>, SR: SignalsRuntimeRef> Clone
	for SignalRc<T, S, SR>
{
	fn clone(&self) -> Self {
		Self {
			strong: self.strong.clone(),
		}
	}
}

impl<T: ?Sized, S: ?Sized + UnmanagedSignal<T, SR>, SR: SignalsRuntimeRef> Clone
	for SignalWeak<T, S, SR>
{
	fn clone(&self) -> Self {
		Self {
			weak: self.weak.clone(),
		}
	}
}

impl<T: ?Sized, S: ?Sized + UnmanagedSignal<T, SR>, SR: SignalsRuntimeRef> Debug
	for SignalRc<T, S, SR>
where
	T: Debug,
{
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		self.strong.clone_runtime_ref().run_detached(|| {
			f.debug_struct("SignalSR")
				.field("(value)", &&**self.strong.read_dyn())
				.finish_non_exhaustive()
		})
	}
}

impl<T: ?Sized, S: ?Sized + UnmanagedSignal<T, SR>, SR: SignalsRuntimeRef> Deref
	for SignalRc<T, S, SR>
{
	type Target = Signal<T, S, SR>;

	fn deref(&self) -> &Self::Target {
		&self.strong
	}
}

impl<T: ?Sized, S: ?Sized + UnmanagedSignal<T, SR>, SR: SignalsRuntimeRef> Borrow<Signal<T, S, SR>>
	for SignalRc<T, S, SR>
{
	fn borrow(&self) -> &Signal<T, S, SR> {
		self.strong.borrow()
	}
}

impl<T: ?Sized, S: ?Sized + UnmanagedSignal<T, SR>, SR: SignalsRuntimeRef> SignalRc<T, S, SR> {
	/// Creates a new [`SignalRc`] from the provided [`UnmanagedSignal`].
	///
	/// For additional constructors, see [`Signal`].
	pub fn new(unmanaged: S) -> Self
	where
		S: Sized,
	{
		SignalRc {
			strong: Strong::pin(unmanaged),
		}
	}

	/// Erases the (generally opaque) type parameter `S`, allowing the signal handle to
	/// be stored easily.
	pub fn into_dyn<'a>(self) -> SignalRcDyn<'a, T, SR>
	where
		S: 'a + Sized,
	{
		let Self { strong } = self;
		SignalRcDyn {
			strong: strong.into_dyn(),
		}
	}

	/// Erases the (generally opaque) type parameter `S`, allowing the signal cell handle
	/// to be stored easily.
	pub fn into_dyn_cell<'a>(self) -> SignalRcDynCell<'a, T, SR>
	where
		S: 'a + Sized + UnmanagedSignalCell<T, SR>,
	{
		let Self { strong } = self;
		SignalRcDynCell {
			strong: strong.into_dyn_cell(),
		}
	}

	/// Subscribes to the managed [`Signal`], converting this [`SignalRc`] into a [`Subscription`].
	///
	/// Compared to [`Signal::to_subscription`], this avoids some memory barriers.
	pub fn into_subscription(self) -> Subscription<T, S, SR> {
		self.strong.managed().subscribe();
		Subscription {
			subscribed: ManuallyDrop::new(self.strong),
		}
	}
}

impl<T: ?Sized, S: Sized + UnmanagedSignalCell<T, SR>, SR: SignalsRuntimeRef> SignalRc<T, S, SR> {
	/// Obscures the cell API, allowing only reads and subscriptions.
	pub fn into_read_only<'a>(self) -> SignalRc<T, impl 'a + UnmanagedSignal<T, SR>, SR>
	where
		S: 'a,
	{
		//FIXME: This is *probably* inefficient.
		self.as_read_only().to_owned()
	}

	/// Equivalent to a getter/setter splitter.
	pub fn into_read_only_and_self<'a>(
		self,
	) -> (SignalRc<T, impl 'a + UnmanagedSignal<T, SR>, SR>, Self)
	where
		S: 'a,
	{
		(self.as_read_only().to_owned(), self)
	}

	/// A getter/setter splitter like [`into_read_only_and_self`](`SignalRc::into_read_only_and_self`),
	/// but additionally type-erases the type parameter `S` for easy storage.
	pub fn into_dyn_read_only_and_self<'a>(
		self,
	) -> (SignalRcDyn<'a, T, SR>, SignalRcDynCell<'a, T, SR>)
	where
		S: 'a,
	{
		(self.as_dyn().to_owned(), self.into_dyn_cell())
	}
}

impl<'a, T: 'a + ?Sized, SR: 'a + SignalsRuntimeRef> SignalRcDynCell<'a, T, SR> {
	/// Obscures the cell API, allowing only reads and subscriptions.
	///
	/// Since 0.1.2.
	pub fn into_read_only(self) -> SignalRcDyn<'a, T, SR> {
		//FIXME: This is *probably* inefficient.
		self.as_read_only().to_owned()
	}

	/// Equivalent to a getter/setter splitter.
	///
	/// Since 0.1.2.
	pub fn into_read_only_and_self(self) -> (SignalRcDyn<'a, T, SR>, Self) {
		(self.clone().into_read_only(), self)
	}
}

impl<T: ?Sized, S: Sized + UnmanagedSignalCell<T, SR>, SR: SignalsRuntimeRef> SignalWeak<T, S, SR> {
	/// Obscures the cell API, allowing only reads and subscriptions.
	#[must_use]
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
	#[must_use]
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
	#[must_use]
	pub fn into_dyn_read_only_and_self<'a>(
		self,
	) -> (SignalWeakDyn<'a, T, SR>, SignalWeakDynCell<'a, T, SR>)
	where
		S: 'a,
	{
		(self.clone().into_dyn(), self.into_dyn_cell())
	}
}

impl<'a, T: 'a + ?Sized, SR: 'a + SignalsRuntimeRef> SignalWeakDynCell<'a, T, SR> {
	/// Obscures the cell API, allowing only reads and subscriptions.
	///
	/// Since 0.1.2.
	#[must_use]
	pub fn into_read_only(self) -> SignalWeakDyn<'a, T, SR> {
		unsafe {
			//SAFETY: Prevents dropping of the original `Weak`,
			//        so that the net count doesn't change.
			let this = ManuallyDrop::new(self);
			SignalWeak {
				weak: this.weak.unsafe_copy().into_read_only(),
			}
		}
	}

	/// Equivalent to a getter/setter splitter.
	///
	/// Since 0.1.2.
	#[must_use]
	pub fn into_read_only_and_self(self) -> (SignalWeakDyn<'a, T, SR>, Self) {
		(self.clone().into_read_only(), self)
	}
}
