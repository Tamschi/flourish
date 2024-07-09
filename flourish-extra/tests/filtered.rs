use flourish::{prelude::*, Signal};

mod _validator;
use _validator::Validator;

mod _block_on;
use _block_on::{assert_pending, assert_ready};
use flourish_extra::future::filtered;

#[test]
fn ready() {
	let v = &Validator::new();

	let signal = Signal::computed(|| v.push("signal"));
	v.expect([]);

	let found = filtered(
		|| {
			v.push("source");
			signal.get()
		},
		|()| {
			v.push("test");
			true
		},
		signal.clone_runtime_ref(),
	);
	v.expect([]);

	let _sub = assert_ready(found);
	v.expect(["source", "signal", "test"])
}

#[test]
fn pending() {
	let v = &Validator::new();

	let signal = Signal::computed(|| v.push("signal"));
	v.expect([]);

	let found = filtered(
		|| {
			v.push("source");
			signal.get()
		},
		|()| {
			v.push("test");
			false
		},
		signal.clone_runtime_ref(),
	);
	v.expect([]);

	let _sub = assert_pending(found);
	v.expect(["source", "signal", "test"])
}
