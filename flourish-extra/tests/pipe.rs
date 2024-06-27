use flourish::{Signal, Subject, SubscriptionSR};
use flourish_extra::{debounce, pipe};

mod _validator;
use _validator::Validator;

#[test]
fn concise() {
    let v = &Validator::new();

    let (get, set) = Subject::new(0).into_get_set();
    let debounced = Signal::uncached(pipe((get, debounce, debounce)));
    let _sub = SubscriptionSR::<_>::new(move || v.push(debounced.get()));
    v.expect([0]);

    for n in [1, 2, 3, 3, 4, 5, 5, 5, 6, 6, 6, 7, 7, 7, 7, 8, 9, 9] {
        set(n);
    }
    v.expect([1, 2, 3, 4, 5, 6, 7, 8, 9]);
}
