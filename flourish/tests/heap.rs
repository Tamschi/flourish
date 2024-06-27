use std::sync::Arc;

use flourish::{shadow_clone, GlobalSignal, GlobalSignalRuntime, Source, Subject, Subscription};
mod _validator;
use _validator::Validator;

#[test]
fn use_constructors() {
    let v = &Validator::new();
    let x = &Validator::new();

    let a = Subject::new(1);
    let (b, set_b) = Subject::new(2).into_get_set();
    let c = GlobalSignal::computed({
        shadow_clone!(a, b);
        move || {
            x.push("c");
            a.get() + b()
        }
    });
    let d = GlobalSignal::computed({
        shadow_clone!(a, b);
        move || {
            x.push("d");
            a.get() - b()
        }
    });
    let aa = Arc::pin({
        shadow_clone!(c, d);
        move || {
            x.push("aa");
            c.get() + d.get()
        }
    });
    v.expect([]);
    x.expect([]);

    let sub_aa = Subscription::new(move || {
        x.push("sub_aa");
        v.push(Source::<GlobalSignalRuntime>::get(aa.as_ref()))
    });
    v.expect([2]);
    x.expect(["sub_aa", "aa", "c", "d"]);

    set_b(2);
    v.expect([2]);
    x.expect(["c", "d", "sub_aa", "aa"]);

    a.set(0);
    v.expect([0]);
    x.expect(["c", "d", "sub_aa", "aa"]);

    drop(sub_aa);

    // These evaluate *no* closures!
    a.set(2);
    set_b(3);
    a.set(5);
    v.expect([]);
    x.expect([]);

    let _sub_c = Subscription::new(move || {
        x.push("_sub_c");
        v.push(c.get())
    });
    v.expect([8]);
    x.expect(["_sub_c", "c"]);

    let _sub_d = Subscription::new(move || {
        x.push("_sub_d");
        v.push(d.get())
    });
    v.expect([2]);
    x.expect(["_sub_d", "d"]);

    a.set(4);
    v.expect([7, 1]);
    x.expect(["c", "d", "_sub_c", "_sub_d"]);
}
