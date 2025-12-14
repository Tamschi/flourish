#![cfg(feature = "local_signals_runtime")]

use flourish_bound::LocalSignalsRuntime;

type Signal<T, S> = flourish_bound::Signal<T, S, LocalSignalsRuntime>;

#[test]
fn direct() {
	let a = Signal::cell(1);
	let b = Signal::computed(|| a.get());

	assert_eq!(b.get(), 1);

	a.replace_blocking(2);
	assert_eq!(b.get(), 2);
}

#[test]
fn indirect() {
	let a = Signal::cell(1);
	let b = Signal::computed(|| a.get());
	let c = Signal::computed(|| b.get());

	assert_eq!(c.get(), 1);

	a.replace_blocking(2);
	assert_eq!(c.get(), 2);
}
