use flourish::{prelude::*, shadow_clone, GlobalSignalRuntime, Signal, SignalCell, Subscription};

mod _validator;
use _validator::Validator;
use flourish_extra::delta;

#[test]
fn delta_test() {
	let v = &Validator::new();

	let (get, set) = SignalCell::new(1)
		.into_getter_and_setter(|s| move || s.get(), |s| move |v| s.replace_blocking(v));
	let delta = Signal::new(delta(get, GlobalSignalRuntime));
	let sub = Subscription::computed({
		shadow_clone!(delta);
		move || v.push(delta.get())
	});
	v.expect([0]);

	for n in [1, 2, 3, 3, 4, 5, 5, 5, 6, 6, 6, 7, 7, 7, 7, 8, 9, 9, 0] {
		set(n);
	}
	v.expect([0, 1, 1, 0, 1, 1, 0, 0, 1, 0, 0, 1, 0, 0, 0, 1, 1, 0, -9]);

	drop(sub);
	set(5);
	set(9);
	v.expect([]);
	let _sub = Subscription::computed(move || v.push(delta.get()));
	v.expect([9]);
	set(9);
	v.expect([0]);
}
