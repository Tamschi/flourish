#![cfg(feature = "local_signals_runtime")]

use flourish_unsend::{LocalSignalsRuntime, SignalsRuntimeRef};

type Effect<'a> = flourish_unsend::Effect<'a, LocalSignalsRuntime>;
type Signal<T, S> = flourish_unsend::Signal<T, S, LocalSignalsRuntime>;

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

	LocalSignalsRuntime.hint_batched_updates(|| {
		validator.expect([]);
		a.set(());
		validator.expect([]);
		b.set(());
		validator.expect([]);
	});
	validator.expect([()]);
}
