use flourish::{raw::computed, GlobalSignalRuntime, Signal, SourcePin as _, Subject, Subscription};
use flourish_extra::debounce;

mod _validator;
use _validator::Validator;

#[test]
fn debounce_test() {
    let v = &Validator::new();
    let x = &Validator::new();

    let (get, set) = Subject::new(0).into_get_set();
    let debounced = Signal::new(debounce(computed(
        move || {
            x.push("d");
            get()
        },
        GlobalSignalRuntime,
    )));
    let _sub = Subscription::new(computed(
        move || {
            x.push("s");
            v.push(debounced.get())
        },
        GlobalSignalRuntime,
    ));
    v.expect([0]);
    x.expect(["s", "d"]);

    let mut previous = 0;
    for n in [1, 2, 3, 3, 4, 5, 5, 5, 6, 6, 6, 7, 7, 7, 7, 8, 9, 9] {
        set(n);
        if n == previous {
            x.expect(["d"]);
        } else {
            x.expect(["d", "s"]);
        }
        previous = n;
    }
    v.expect([1, 2, 3, 4, 5, 6, 7, 8, 9]);
}
