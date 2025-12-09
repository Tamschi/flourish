#![cfg(feature = "global_signals_runtime")]

use flourish::{shadow_clone, GlobalSignalsRuntime, Propagation};

type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
type Subscription<T, S> = flourish::Subscription<T, S, GlobalSignalsRuntime>;

mod _validator;
use _validator::Validator;

#[test]
fn intrinsic() {
	let v = &Validator::new();

	let a = Signal::cell_reactive((), |_value, status| {
		v.push(status);
		Propagation::Halt
	});
	let s = a.as_read_only().to_owned();
	drop(a);
	v.expect([]);

	let s = s.to_subscription();
	v.expect([true]);

	drop(s);
	v.expect([false]);
}

#[test]
fn dependent() {
	let v = &Validator::new();

	let a = Signal::cell_reactive((), |_value, status| {
		v.push(status);
		Propagation::Halt
	});
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

	let a = Signal::cell_reactive((), |_value, status| {
		v.push(status);
		Propagation::Halt
	});
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

#[test]
fn lifecycle() {
	let v = &Validator::new();

	let s = Signal::cell_reactive_mut(false, |value, status| {
		*value = status;
		match status {
			true => Propagation::Propagate,
			false => Propagation::FlushOut,
		}
	})
	.into_read_only();
	assert!(!s.get());

	let s = Signal::computed(move || v.push(s.get()));
	v.expect([]);

	let s = s.to_subscription();
	v.expect([true]);

	let s = s.unsubscribe();
	v.expect([false]);

	drop(s);
	v.expect([]);
}
