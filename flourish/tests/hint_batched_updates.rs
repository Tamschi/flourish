#![cfg(feature = "global_signals_runtime")]

use flourish::{GlobalSignalsRuntime, SignalsRuntimeRef};

type Effect<'a> = flourish::Effect<'a, GlobalSignalsRuntime>;
type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;

mod _validator;
use _validator::Validator;

#[test]
fn deduplication() {
	let validator = &Validator::new();

	let a = Signal::cell(());
	let b = Signal::cell(());
	let _e = Effect::new(|| (a.get(), b.get()).1, |()| validator.push(()));

	validator.expect([]);

	a.set(());
	b.set(());
	validator.expect([(), ()]);

	GlobalSignalsRuntime.hint_batched_updates(|| {
		validator.expect([]);
		a.set(());
		validator.expect([]);
		b.set(());
		validator.expect([]);
	});
	validator.expect([()]);
}
