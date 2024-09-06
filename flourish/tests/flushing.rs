#![cfg(feature = "global_signals_runtime")]

use flourish::{GlobalSignalsRuntime, Propagation};

type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
type Effect<'a> = flourish::Effect<'a, GlobalSignalsRuntime>;

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
