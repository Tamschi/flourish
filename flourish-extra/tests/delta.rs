#![cfg(feature = "_test")]

use flourish::{prelude::*, shadow_clone, GlobalSignalsRuntime, Signal, SignalCell, SubscriptionArc_};

mod _validator;
use _validator::Validator;
use flourish_extra::delta;

#[test]
fn delta_test() {
	let v = &Validator::new();

	let (signal, cell) = Signal::cell(1).into_signal_and_self_dyn();
	let delta = Signal::new(delta(move || signal.get(), GlobalSignalsRuntime));
	let sub = SubscriptionArc_::computed({
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
	let _sub = SubscriptionArc_::computed(move || v.push(delta.get()));
	v.expect([9]);
	cell.replace_blocking(9);
	v.expect([0]);
}
