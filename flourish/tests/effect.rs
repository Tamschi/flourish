#![cfg(feature = "global_signal_runtime")]

use flourish::{prelude::*, Effect, SignalCell};
mod _validator;
use _validator::Validator;

#[test]
fn heap() {
	let v = &Validator::new();

	let (a, a_cell) = SignalCell::new(()).into_signal_and_erased();

	let e = Effect::new(
		move || {
			a.get();
			v.push("f")
		},
		|()| v.push("drop"),
	);
	v.expect(["f"]);

	a_cell.replace_blocking(());
	v.expect(["drop", "f"]);

	drop(e);
	v.expect(["drop"]);

	drop(a_cell);
	v.expect([]);
}

//TODO: Ensure that flushing doesn't cause issues when dropping an Effect!
