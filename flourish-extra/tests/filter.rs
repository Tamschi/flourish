use flourish::{raw::computed_uncached, Signal, SourcePin as _};
use flourish_extra::future::filter_from_source;

mod _validator;
use _validator::Validator;

mod _block_on;
use _block_on::{assert_pending, assert_ready};

#[test]
fn ready() {
    let v = &Validator::new();

    let signal = Signal::computed(|| v.push("signal"));
    v.expect([]);

    let found = filter_from_source(
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

    let _sub = assert_ready(found);
    v.expect(["source", "signal", "test"])
}

#[test]
fn pending() {
    let v = &Validator::new();

    let signal = Signal::computed(|| v.push("signal"));
    v.expect([]);

    let found = filter_from_source(
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

    let _sub = assert_pending(found);
    v.expect(["source", "signal", "test"])
}