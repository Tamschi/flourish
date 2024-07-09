use flourish::{prelude::*, Effect, SignalCell, SourcePin};
mod _validator;
use _validator::Validator;

#[test]
fn heap() {
	let v = &Validator::new();

	let (a, set_a) = SignalCell::new(())
		.into_getter_and_setter(|s| move || s.get(), |s| move |v| s.replace_blocking(v));

	let e = Effect::new(
		move || {
			a();
			v.push("f")
		},
		|()| v.push("drop"),
	);
	v.expect(["f"]);

	set_a(());
	v.expect(["drop", "f"]);

	drop(e);
	v.expect(["drop"]);

	drop(set_a);
	v.expect([]);
}
