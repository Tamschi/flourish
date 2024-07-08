use flourish::{raw::computed, GlobalSignalRuntime, Signal, SourcePin as _, Announcer, Subscription};
use flourish_extra::{debounce_from_source, pipe};

mod _validator;
use _validator::Validator;

#[test]
fn concise() {
    let v = &Validator::new();

    let (get, set) = Announcer::new(0)
        .into_mapped_source_sender(|s| move || s.get(), |s| move |v| s.replace_blocking(v));
    let debounced = Signal::new(pipe((
        computed(get, GlobalSignalRuntime),
        debounce_from_source,
        debounce_from_source,
    )));
    let _sub = Subscription::computed(move || v.push(debounced.get()));
    v.expect([0]);

    for n in [1, 2, 3, 3, 4, 5, 5, 5, 6, 6, 6, 7, 7, 7, 7, 8, 9, 9] {
        set(n);
    }
    v.expect([1, 2, 3, 4, 5, 6, 7, 8, 9]);
}
