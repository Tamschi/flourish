#![cfg(feature = "global_signals_runtime")]

use flourish::{prelude::*, shadow_clone, Effect, SignalCell, SourcePin, SubscriptionArc_};
mod _validator;
use _validator::Validator;

#[test]
fn set() {
	let v = &Validator::new();

	let a = Signal::cell("a");
	let b = Signal::cell("b");
	let _sub_a = SubscriptionArc_::computed({
		shadow_clone!(a);
		move || v.push(("_sub_a", a.get()))
	});
	let _sub_b = SubscriptionArc_::computed({
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
