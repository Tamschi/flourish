use flourish::{raw::computed_uncached, Signal, SourcePin as _};
use flourish_extra::future::{flatten_some, flatten_some_from_source};

mod _validator;
use _validator::Validator;

mod _block_on;
use _block_on::{assert_pending, assert_ready};

#[test]
fn ready() {
	let v = &Validator::new();

	let signal = Signal::computed(|| Some(v.push("signal")));
	v.expect([]);

	let found = flatten_some(
		|| {
			v.push("source");
			signal.get()
		},
		signal.clone_runtime_ref(),
	);
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

	let found = flatten_some(
		|| {
			v.push("source");
			signal.get()
		},
		signal.clone_runtime_ref(),
	);
	v.expect([]);

	let _sub = assert_pending(found);
	v.expect(["source", "signal"])
}

#[test]
fn ready_from_source() {
	let v = &Validator::new();

	let signal = Signal::computed(|| Some(v.push("signal")));
	v.expect([]);

	let found = flatten_some_from_source(computed_uncached(
		|| {
			v.push("source");
			signal.get()
		},
		signal.clone_runtime_ref(),
	));
	v.expect([]);

	let _sub = assert_ready(found);
	v.expect(["source", "signal"])
}

#[test]
fn pending_from_source() {
	let v = &Validator::new();

	let signal = Signal::computed(|| {
		v.push("signal");
		None::<()>
	});
	v.expect([]);

	let found = flatten_some_from_source(computed_uncached(
		|| {
			v.push("source");
			signal.get()
		},
		signal.clone_runtime_ref(),
	));
	v.expect([]);

	let _sub = assert_pending(found);
	v.expect(["source", "signal"])
}
