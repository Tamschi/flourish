#![cfg(feature = "global_signals_runtime")]

use flourish::{prelude::*, shadow_clone, Propagation, Signal, SignalCell, Subscription};
mod _validator;
use _validator::Validator;

#[test]
fn inherent() {
	let v = &Validator::new();

	let a = SignalCell::new_reactive((), |_value, status| {
		v.push(status);
		Propagation::Halt
	});
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

	let a = SignalCell::new_reactive((), |_value, status| {
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

	let a = SignalCell::new_reactive((), |_value, status| {
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

	let (s, _) = SignalCell::new_reactive_mut(false, |value, status| {
		*value = status;
		Propagation::Propagate
	})
	.into_signal_and_self();
	assert!(!s.get());

	let s = Signal::computed(move || v.push(s.get()));
	v.expect([]);

	let s = s.try_subscribe().unwrap();
	v.expect([true]);

	let s = s.unsubscribe();
	v.expect([false]);

	drop(s);
	v.expect([]);
}
