use flourish::{prelude::*, shadow_clone, Effect, SignalCell};
mod _validator;
use _validator::Validator;

#[test]
fn cyclic() {
	let v = &Validator::new();

	let p = SignalCell::new_cyclic((), |weak_signal_cell| {
		move |status| {
			v.push((weak_signal_cell.upgrade().is_some(), status));
		}
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
