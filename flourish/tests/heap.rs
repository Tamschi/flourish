use flourish::{shadow_clone, Signal, Subject, Subscription};
mod _validator;
use _validator::Validator;

#[test]
fn use_constructors() {
    let v = &Validator::new();

    let a = Subject::new(1);
    let (b, set_b) = Subject::new(2).into_get_set();
    let c = Signal::new({
        shadow_clone!(a, b);
        move || a.get() + b()
    });
    let d = Signal::new({
        shadow_clone!(a, b);
        move || a.get() - b()
    });
    let aa = Signal::new({
        shadow_clone!(c, d);
        move || c.get() + d.get()
    }); //TODO: Make this a cacheless signal.
    v.expect([]);

    let sub = Subscription::new(move || v.push(aa.get()));
    v.expect([2]);

    set_b(2);
    v.expect([2]);

    a.set(0);
    v.expect([0]);

    drop(sub);

    // These evaluate *no* closures!
    a.set(2);
    set_b(3);
    a.set(5);
    v.expect([]);

    let _sub_c = Subscription::new(move || v.push(c.get()));
    v.expect([8]);

    let _sub_d = Subscription::new(move || v.push(d.get()));
    v.expect([2]);

    a.set(4);
    v.expect([7, 1]);
}
