#![cfg(feature = "global_signals_runtime")]

use flourish::{prelude::*, Effect, Propagation, Signal, SignalCell};
mod _validator;
use _validator::Validator;

#[test]
fn flushing() {
	let seen = &Validator::new();

	let a = SignalCell::new_reactive_mut(false, |value, status| {
		*value = status;
		Propagation::Propagate
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

//TODO: Make flushing affect unsubscribed dependents too!
//      (Maybe with a Propagation::FlushOut variant that transitively affects unsubscribed dependencies unless stopped by Propagation::Halt?)
