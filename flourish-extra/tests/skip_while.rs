use flourish::{raw::computed_uncached, Signal, SourcePin as _};
use flourish_extra::future::{skip_while, skip_while_from_source, skip_while_from_source_cloned};

mod _validator;
use _validator::Validator;

mod _block_on;
use _block_on::{assert_pending, assert_ready};

#[test]
fn ready() {
	let v = &Validator::new();

	let signal = Signal::computed(|| v.push("signal"));
	v.expect([]);

	let found = skip_while(
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

	let _sub = assert_ready(found);
	v.expect(["source", "signal", "test"])
}

#[test]
fn ready_from_source() {
	let v = &Validator::new();

	let signal = Signal::computed(|| v.push("signal"));
	v.expect([]);

	let found = skip_while_from_source(
		computed_uncached(
			|| {
				v.push("source");
				signal.get()
			},
			signal.clone_runtime_ref(),
		),
		|()| {
			v.push("test");
			false
		},
	);
	v.expect([]);

	let _sub = assert_ready(found);
	v.expect(["source", "signal", "test"])
}

#[test]
fn ready_cloned() {
	let v = &Validator::new();

	let signal = Signal::computed(|| v.push("signal"));
	v.expect([]);

	let found = skip_while_from_source_cloned(
		computed_uncached(
			|| {
				v.push("source");
				signal.get()
			},
			signal.clone_runtime_ref(),
		),
		|()| {
			v.push("test");
			false
		},
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

	let found = skip_while(
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

	let _sub = assert_pending(found);
	v.expect(["source", "signal", "test"])
}

#[test]
fn pending_from_source() {
	let v = &Validator::new();

	let signal = Signal::computed(|| v.push("signal"));
	v.expect([]);

	let found = skip_while_from_source(
		computed_uncached(
			|| {
				v.push("source");
				signal.get()
			},
			signal.clone_runtime_ref(),
		),
		|()| {
			v.push("test");
			true
		},
	);
	v.expect([]);

	let _sub = assert_pending(found);
	v.expect(["source", "signal", "test"])
}

#[test]
fn pending_cloned() {
	let v = &Validator::new();

	let signal = Signal::computed(|| v.push("signal"));
	v.expect([]);

	let found = skip_while_from_source_cloned(
		computed_uncached(
			|| {
				v.push("source");
				signal.get()
			},
			signal.clone_runtime_ref(),
		),
		|()| {
			v.push("test");
			true
		},
	);
	v.expect([]);

	let _sub = assert_pending(found);
	v.expect(["source", "signal", "test"])
}
