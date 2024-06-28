use flourish::{shadow_clone, Signal, Subject, Subscription};
use flourish_extra::delta;

mod _validator;
use _validator::Validator;

#[test]
fn delta_test() {
    let v = &Validator::new();

    let (get, set) = Subject::new(1).into_get_set();
    let delta = Signal::uncached(delta(get));
    let sub = Subscription::new({
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
    let _sub = Subscription::new(move || v.push(delta.get()));
    v.expect([9]);
    set(9);
    v.expect([0]);
}
