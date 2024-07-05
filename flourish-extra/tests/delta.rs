use flourish::{
    raw::computed_uncached, shadow_clone, GlobalSignalRuntime, Signal, SourcePin as _, SubjectSR,
    Subscription,
};
use flourish_extra::delta_from_source;

mod _validator;
use _validator::Validator;

#[test]
fn delta_test() {
    let v = &Validator::new();

    let (get, set) = SubjectSR::new(1).into_get_set();
    let delta = Signal::new(delta_from_source(computed_uncached(
        get,
        GlobalSignalRuntime,
    )));
    let sub = Subscription::computed({
        shadow_clone!(delta);
        move || v.push(delta.get())
    });
    v.expect([0]);

    for n in [1, 2, 3, 3, 4, 5, 5, 5, 6, 6, 6, 7, 7, 7, 7, 8, 9, 9, 0] {
        set(n);
    }
    v.expect([0, 1, 1, 0, 1, 1, 0, 0, 1, 0, 0, 1, 0, 0, 0, 1, 1, 0, -9]);

    drop(sub);
    set(5);
    set(9);
    v.expect([]);
    let _sub = Subscription::computed(move || v.push(delta.get()));
    v.expect([9]);
    set(9);
    v.expect([0]);
}
