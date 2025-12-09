#![cfg(feature = "global_signals_runtime")]

use flourish_bound::{shadow_clone, shadow_ref_to_owned, GlobalSignalsRuntime, Propagation};

type Effect<'a> = flourish_bound::Effect<'a, GlobalSignalsRuntime>;
type Signal<T, S> = flourish_bound::Signal<T, S, GlobalSignalsRuntime>;

mod _validator;
use _validator::Validator;

#[test]
fn cyclic() {
	let v = &Validator::new();

	let p = Signal::cell_cyclic_reactive(|weak_signal_cell| {
		shadow_ref_to_owned!(weak_signal_cell);
		((), move |_value, status| {
			v.push((weak_signal_cell.upgrade().is_some(), status));
			Propagation::Halt
		})
	});

	let e = Effect::new(
		{
			shadow_clone!(p);
			move || p.get()
		},
		drop,
	);
	v.expect([(true, true)]);

	drop(p);
	v.expect([]);

	drop(e);
	v.expect([(true, false)]);
}
