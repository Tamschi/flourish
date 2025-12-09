#![cfg(feature = "global_signals_runtime")]

use flourish_bound::{GlobalSignalsRuntime, SignalsRuntimeRef};

type Effect<'a> = flourish_bound::Effect<'a, GlobalSignalsRuntime>;
type Signal<T, S> = flourish_bound::Signal<T, S, GlobalSignalsRuntime>;

mod _validator;
use _validator::Validator;

#[test]
fn deduplication() {
	let validator = &Validator::new();

	let a = Signal::cell(());
	let b = Signal::cell(());
	let _e = Effect::new(|| (a.get(), b.get()).1, |()| validator.push(()));

	validator.expect([]);

	a.replace(());
	b.replace(());
	validator.expect([(), ()]);

	GlobalSignalsRuntime.hint_batched_updates(|| {
		validator.expect([]);
		a.replace(());
		validator.expect([]);
		b.replace(());
		validator.expect([]);
	});
	validator.expect([()]);
}
