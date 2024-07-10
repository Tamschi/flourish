use flourish::{prelude::*, shadow_clone, SignalCell, Subscription};
mod _validator;
use _validator::Validator;

#[test]
fn inherent() {
	let v = &Validator::new();

	let a = SignalCell::new_reactive((), |_value, status| v.push(status));
	let s = a.to_signal();
	drop(a);
	v.expect([]);

	let s = s.try_subscribe().unwrap();
	v.expect([true]);

	drop(s);
	v.expect([false]);
}

#[test]
fn dependent() {
	let v = &Validator::new();

	let a = SignalCell::new_reactive((), |_value, status| v.push(status));
	v.expect([]);

	let s = Subscription::computed({
		shadow_clone!(a);
		move || a.get()
	});
	v.expect([true]);

	drop(a);
	v.expect([]);

	drop(s);
	v.expect([false]);
}

#[test]
fn dependent_reversed() {
	let v = &Validator::new();

	let a = SignalCell::new_reactive((), |_value, status| v.push(status));
	v.expect([]);

	let s = Subscription::computed({
		shadow_clone!(a);
		move || a.get()
	});
	v.expect([true]);

	drop(s);
	v.expect([false]);

	drop(a);
	v.expect([]);
}
