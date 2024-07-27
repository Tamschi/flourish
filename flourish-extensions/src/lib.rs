#![warn(clippy::pedantic)]
#![warn(unreachable_pub)]

mod opaque;

#[allow(async_fn_in_trait)]
pub mod prelude {
	use std::ops::{AddAssign, Sub};

	use ext_trait::extension;
	use flourish::{unmanaged::Subscribable, ArcSignal, SignalsRuntimeRef, ArcSubscription};
	use flourish_extra::{
		delta,
		future::{filter_mapped, filtered, skipped_while},
		sparse_tally,
	};
	use num_traits::Zero;

	use crate::opaque::Opaque;

	//TODO: These have extraneous bounds that aren't really needed, usually `T: Sync + Copy`.

	#[extension(pub trait SignalExt)]
	impl<'a, T: 'a + Send + ?Sized, SR: 'a + SignalsRuntimeRef> ArcSignal<T, Opaque, SR> {
		fn delta<V: 'a + Send>(
			fn_pin: impl 'a + Send + FnMut() -> V,
		) -> ArcSignal<T, impl Sized + Subscribable<T, SR>, SR>
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
		) -> ArcSignal<T, impl Sized + Subscribable<T, SR>, SR>
		where
			T: Zero,
			for<'b> &'b V: Sub<Output = T>,
		{
			ArcSignal::new(delta(fn_pin, runtime))
		}

		fn sparse_tally<V: 'a + Send>(
			fn_pin: impl 'a + Send + FnMut() -> V,
		) -> ArcSignal<T, impl Sized + Subscribable<T, SR>, SR>
		where
			T: Zero + Send + AddAssign<V>,
			SR: Default,
		{
			Self::sparse_tally_with_runtime(fn_pin, SR::default())
		}

		fn sparse_tally_with_runtime<V: 'a + Send>(
			fn_pin: impl 'a + Send + FnMut() -> V,
			runtime: SR,
		) -> ArcSignal<T, impl Sized + Subscribable<T, SR>, SR>
		where
			T: Zero + Send + AddAssign<V>,
		{
			ArcSignal::new(sparse_tally(fn_pin, runtime))
		}
	}

	#[extension(pub trait SubscriptionExt)]
	impl<'a, T: 'a + Send + Sync + ?Sized + Clone, SR: 'a + SignalsRuntimeRef>
		ArcSubscription<T, Opaque, SR>
	{
		async fn skipped_while(
			fn_pin: impl 'a + Send + FnMut() -> T,
			predicate_fn_pin: impl 'a + Send + FnMut(&T) -> bool,
		) -> ArcSubscription<T, impl Sized + Subscribable<T, SR>, SR>
		where
			SR: Default,
		{
			Self::skipped_while_with_runtime(fn_pin, predicate_fn_pin, SR::default()).await
		}

		async fn skipped_while_with_runtime(
			fn_pin: impl 'a + Send + FnMut() -> T,
			predicate_fn_pin: impl 'a + Send + FnMut(&T) -> bool,
			runtime: SR,
		) -> ArcSubscription<T, impl Sized + Subscribable<T, SR>, SR> {
			skipped_while(fn_pin, predicate_fn_pin, runtime).await
		}

		async fn filtered(
			fn_pin: impl 'a + Send + FnMut() -> T,
			predicate_fn_pin: impl 'a + Send + FnMut(&T) -> bool,
		) -> ArcSubscription<T, impl Sized + Subscribable<T, SR>, SR>
		where
			T: Copy,
			SR: Default,
		{
			Self::filtered_with_runtime(fn_pin, predicate_fn_pin, SR::default()).await
		}

		async fn filtered_with_runtime(
			fn_pin: impl 'a + Send + FnMut() -> T,
			predicate_fn_pin: impl 'a + Send + FnMut(&T) -> bool,
			runtime: SR,
		) -> ArcSubscription<T, impl Sized + Subscribable<T, SR>, SR>
		where
			T: Copy,
		{
			filtered(fn_pin, predicate_fn_pin, runtime).await
		}

		async fn filter_mapped(
			fn_pin: impl 'a + Send + FnMut() -> Option<T>,
		) -> ArcSubscription<T, impl Sized + Subscribable<T, SR>, SR>
		where
			T: Copy,
			SR: Default,
		{
			Self::filter_mapped_with_runtime(fn_pin, SR::default()).await
		}

		async fn filter_mapped_with_runtime(
			fn_pin: impl 'a + Send + FnMut() -> Option<T>,
			runtime: SR,
		) -> ArcSubscription<T, impl Sized + Subscribable<T, SR>, SR>
		where
			T: Copy,
		{
			filter_mapped(fn_pin, runtime).await
		}
	}
}
