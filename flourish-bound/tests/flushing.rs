#![cfg(feature = "local_signals_runtime")]

use flourish_bound::{LocalSignalsRuntime, Propagation};

type Signal<T, S> = flourish_bound::Signal<T, S, LocalSignalsRuntime>;
type Effect<'a> = flourish_bound::Effect<'a, LocalSignalsRuntime>;

mod _validator;
use _validator::Validator;

#[test]
fn not_flushed() {
	let seen = &Validator::new();

	let a = Signal::cell_reactive_mut(false, |value, status| {
		*value = status;
		Propagation::Propagate
	});
	let s = Signal::computed(|| seen.push(a.get()));
	seen.expect([]);

	let e = Effect::new(|| s.get(), drop);
	seen.expect([true]);

	drop(e);
	seen.expect([]);

	drop(s);
	drop(a);
	seen.expect([]);
}

#[test]
fn flushed() {
	let seen = &Validator::new();

	let a = Signal::cell_reactive_mut(false, |value, status| {
		*value = status;
		Propagation::FlushOut
	});
	let s = Signal::computed(|| seen.push(a.get()));
	seen.expect([]);

	let e = Effect::new(|| s.get(), drop);
	seen.expect([true]);

	drop(e);
	seen.expect([false]);

	drop(s);
	drop(a);
	seen.expect([]);
}

#[test]
fn opportunistic_skip() {
	let seen = &Validator::new();

	let a = Signal::cell_reactive_mut(false, |value, status| {
		*value = status;
		Propagation::FlushOut
	});
	let s = Signal::computed(|| seen.push(a.get()));
	seen.expect([]);

	let s = s.into_subscription();
	seen.expect([true]);

	drop(s);
	drop(a);
	seen.expect([]);
}

#[test]
fn no_skip_if_not_exclusive() {
	let seen = &Validator::new();

	let a = Signal::cell_reactive_mut(false, |value, status| {
		*value = status;
		Propagation::FlushOut
	});
	let s = Signal::computed(|| seen.push(a.get()));
	seen.expect([]);

	let sub = s.to_subscription();
	seen.expect([true]);

	drop(sub);
	seen.expect([false]);

	drop(s);
	drop(a);
	seen.expect([]);
}
