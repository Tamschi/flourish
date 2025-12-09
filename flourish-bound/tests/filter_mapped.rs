#![cfg(feature = "_test")]

use flourish::GlobalSignalsRuntime;

type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
type Subscription<T, S> = flourish::Subscription<T, S, GlobalSignalsRuntime>;

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
