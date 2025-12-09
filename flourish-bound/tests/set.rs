#![cfg(feature = "global_signals_runtime")]

use flourish_bound::{shadow_clone, GlobalSignalsRuntime};

type Effect<'a> = flourish_bound::Effect<'a, GlobalSignalsRuntime>;
type Signal<T, S> = flourish_bound::Signal<T, S, GlobalSignalsRuntime>;
type Subscription<T, S> = flourish_bound::Subscription<T, S, GlobalSignalsRuntime>;

mod _validator;
use _validator::Validator;

#[test]
fn set() {
	let v = &Validator::new();

	let a = Signal::cell("a");
	let b = Signal::cell("b");
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
