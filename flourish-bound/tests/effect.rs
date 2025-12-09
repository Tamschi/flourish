#![cfg(feature = "global_signals_runtime")]

use flourish_bound::{GlobalSignalsRuntime, Propagation};

type Effect<'a> = flourish_bound::Effect<'a, GlobalSignalsRuntime>;
type Signal<T, S> = flourish_bound::Signal<T, S, GlobalSignalsRuntime>;

mod _validator;
use _validator::Validator;

#[test]
fn heap() {
	let v = &Validator::new();

	let (a, a_cell) = Signal::cell(()).into_dyn_read_only_and_self();

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

#[test]
fn effect_drop_is_distinct() {
	let constructions = &Validator::new();
	let destructions = &Validator::new();

	let a = Signal::cell_reactive((), |_value, _status| Propagation::Propagate);
	let e = Effect::new(
		|| constructions.push(a.get()),
		|value| destructions.push(value),
	);
	constructions.expect([()]);
	destructions.expect([]);

	drop(e);
	constructions.expect([]);
	destructions.expect([()]);
}
