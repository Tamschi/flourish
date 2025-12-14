#![cfg(feature = "_test")]

use flourish_bound::LocalSignalsRuntime;

type Signal<T, S> = flourish_bound::Signal<T, S, LocalSignalsRuntime>;
type Subscription<T, S> = flourish_bound::Subscription<T, S, LocalSignalsRuntime>;

mod _validator;
use _validator::Validator;

mod _block_on;
use _block_on::{assert_pending, assert_ready};

#[test]
fn ready() {
	let v = &Validator::new();

	let signal = Signal::computed(|| Some(v.push("signal")));
	v.expect([]);

	let found = Subscription::filter_mapped(|| {
		v.push("source");
		signal.get()
	});
	v.expect([]);

	let _sub = assert_ready(found);
	v.expect(["source", "signal"])
}

#[test]
fn pending() {
	let v = &Validator::new();

	let signal = Signal::computed(|| {
		v.push("signal");
		None::<()>
	});
	v.expect([]);

	let found = Subscription::filter_mapped(|| {
		v.push("source");
		signal.get()
	});
	v.expect([]);

	let _sub = assert_pending(found);
	v.expect(["source", "signal"])
}
