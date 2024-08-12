#![cfg(feature = "_test")]

use flourish::{shadow_clone, GlobalSignalsRuntime};

type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
type Subscription<T, S> = flourish::Subscription<T, S, GlobalSignalsRuntime>;

mod _validator;
use _validator::Validator;
use flourish_extra::delta;

#[test]
fn delta_test() {
	let v = &Validator::new();

	let (signal, cell) = Signal::cell(1).into_dyn_and_dyn_cell();
	let delta = Signal::new(delta(move || signal.get(), GlobalSignalsRuntime));
	let sub = Subscription::computed({
		shadow_clone!(delta);
		move || v.push(delta.get())
	});
	v.expect([0]);

	for n in [1, 2, 3, 3, 4, 5, 5, 5, 6, 6, 6, 7, 7, 7, 7, 8, 9, 9, 0] {
		cell.replace_blocking(n);
	}
	v.expect([0, 1, 1, 0, 1, 1, 0, 0, 1, 0, 0, 1, 0, 0, 0, 1, 1, 0, -9]);

	drop(sub);
	cell.replace_blocking(5);
	cell.replace_blocking(9);
	v.expect([]);
	let _sub = Subscription::computed(move || v.push(delta.get()));
	v.expect([9]);
	cell.replace_blocking(9);
	v.expect([0]);
}
