use flourish::{prelude::*, shadow_clone, Effect, SignalCell, Update};
mod _validator;
use _validator::Validator;

#[test]
fn cyclic() {
	let v = &Validator::new();

	let p = SignalCell::new_cyclic_reactive(|weak_signal_cell| {
		((), move |_value, status| {
			v.push((weak_signal_cell.upgrade().is_some(), status));
			Update::Halt
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
