use flourish::{prelude::*, shadow_clone, Effect, SignalCell, SourcePin, Subscription};
mod _validator;
use _validator::Validator;

#[test]
fn set() {
	let v = &Validator::new();

	let a = SignalCell::new("a");
	let b = SignalCell::new("b");
	let _sub_a = Subscription::computed({
		shadow_clone!(a);
		move || v.push(("_sub_a", a.get()))
	});
	let _sub_b = Subscription::computed({
		shadow_clone!(b);
		move || v.push(("_sub_b", b.get()))
	});
	let _effect = Effect::new(
		{
			shadow_clone!(a, b);
			move || b.replace(a.get())
		},
		drop,
	);
	v.expect([("_sub_a", "a"), ("_sub_b", "b"), ("_sub_b", "a")]);

	a.replace_blocking("aa");

	v.expect([("_sub_a", "aa"), ("_sub_b", "aa")]);
}
