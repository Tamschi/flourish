#![warn(clippy::pedantic)]
#![warn(unreachable_pub)]

mod opaque;

#[allow(async_fn_in_trait)]
pub mod prelude {
	use std::ops::{AddAssign, Sub};

	use ext_trait::extension;
	use flourish::{unmanaged::Subscribable, SignalArc, SignalsRuntimeRef};
	use flourish_extra::{delta, sparse_tally};
	use num_traits::Zero;

	use crate::opaque::Opaque;

	//TODO: These have extraneous bounds that aren't really needed, usually `T: Sync + Copy`.

	#[extension(pub trait SignalExt)]
	impl<'a, T: 'a + Send + ?Sized, SR: 'a + SignalsRuntimeRef> SignalArc<T, Opaque, SR> {
		fn delta<V: 'a + Send>(
			fn_pin: impl 'a + Send + FnMut() -> V,
		) -> SignalArc<T, impl Sized + Subscribable<T, SR>, SR>
		where
			T: Zero,
			for<'b> &'b V: Sub<Output = T>,
			SR: Default,
		{
			Self::delta_with_runtime(fn_pin, SR::default())
		}

		fn delta_with_runtime<V: 'a + Send>(
			fn_pin: impl 'a + Send + FnMut() -> V,
			runtime: SR,
		) -> SignalArc<T, impl Sized + Subscribable<T, SR>, SR>
		where
			T: Zero,
			for<'b> &'b V: Sub<Output = T>,
		{
			SignalArc::new(delta(fn_pin, runtime))
		}

		fn sparse_tally<V: 'a + Send>(
			fn_pin: impl 'a + Send + FnMut() -> V,
		) -> SignalArc<T, impl Sized + Subscribable<T, SR>, SR>
		where
			T: Zero + Send + AddAssign<V>,
			SR: Default,
		{
			Self::sparse_tally_with_runtime(fn_pin, SR::default())
		}

		fn sparse_tally_with_runtime<V: 'a + Send>(
			fn_pin: impl 'a + Send + FnMut() -> V,
			runtime: SR,
		) -> SignalArc<T, impl Sized + Subscribable<T, SR>, SR>
		where
			T: Zero + Send + AddAssign<V>,
		{
			SignalArc::new(sparse_tally(fn_pin, runtime))
		}
	}
}
