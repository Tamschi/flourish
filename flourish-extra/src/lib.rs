#![warn(clippy::pedantic)]
#![warn(unreachable_pub)]

use std::ops::{AddAssign, Sub};

use flourish::{
	unmanaged::{folded, Subscribable},
	Propagation, SignalsRuntimeRef, SubscriptionSR,
};
use num_traits::Zero;

pub mod future;

//BLOCKED: `reduce`, `filter` and `fold` (as curried operators) wait on <https://github.com/rust-lang/rust/issues/99697>.

//TODO: These have extraneous bounds. Change to accept closures to remove some `T: Sync + Copy` bounds.

pub fn delta<'a, V: 'a + Send, T: 'a + Send + Zero, SR: 'a + SignalsRuntimeRef>(
	mut fn_pin: impl 'a + Send + FnMut() -> V,
	runtime: SR,
) -> impl 'a + Subscribable<T, SR>
where
	for<'b> &'b V: Sub<Output = T>,
{
	let mut previous = None;
	folded(
		<&'a V as Sub>::Output::zero(),
		move |delta| {
			let next: V = fn_pin();
			if let Some(previous) = previous.as_mut() {
				*delta = &next - &*previous;
			}
			previous = Some(next);
			Propagation::Propagate
		},
		runtime,
	)
}

pub fn sparse_tally<'a, V: 'a, T: 'a + Zero + Send + AddAssign<V>, SR: 'a + SignalsRuntimeRef>(
	mut fn_pin: impl 'a + Send + FnMut() -> V,
	runtime: SR,
) -> impl 'a + Subscribable<T, SR> {
	folded(
		T::zero(),
		move |tally| {
			*tally += fn_pin();
			Propagation::Propagate
		},
		runtime,
	)
}

pub fn eager_tally<'a, V: 'a, T: 'a + Zero + Send + AddAssign<V>, SR: 'a + SignalsRuntimeRef>(
	fn_pin: impl 'a + Send + FnMut() -> V,
	runtime: SR,
) -> SubscriptionSR<T, impl 'a + Subscribable<T, SR>, SR> {
	SubscriptionSR::new(sparse_tally(fn_pin, runtime))
}
