use flourish::{shadow_clone, Announcer, Signal, SourcePin as _, Subscription};
mod _validator;
use _validator::Validator;

#[test]
fn use_constructors() {
    let v = &Validator::new();
    let x = &Validator::new();

    let a = Announcer::new(1);
    let (b, set_b) = Announcer::new(2)
        .into_getter_and_setter(|s| move || s.get(), |s| move |v| s.replace_blocking(v));
    let c = Signal::computed({
        shadow_clone!(a, b);
        move || {
            x.push("c");
            a.get() + b()
        }
    });
    let d = Signal::computed({
        shadow_clone!(a, b);
        move || {
            x.push("d");
            a.get() - b()
        }
    });
    let aa = Signal::computed_uncached({
        shadow_clone!(c, d);
        move || {
            x.push("aa");
            c.get() + d.get()
        }
    });
    v.expect([]);
    x.expect([]);

    let sub_aa = Subscription::computed(move || {
        x.push("sub_aa");
        v.push(aa.get())
    });
    v.expect([2]);
    x.expect(["sub_aa", "aa", "c", "d"]);

    set_b(2);
    v.expect([2]);
    x.expect(["c", "d", "sub_aa", "aa"]);

    a.replace_blocking(0);
    v.expect([0]);
    x.expect(["c", "d", "sub_aa", "aa"]);

    drop(sub_aa);

    // These evaluate *no* closures!
    a.replace_blocking(2);
    set_b(3);
    a.replace_blocking(5);
    v.expect([]);
    x.expect([]);

    let _sub_c = Subscription::computed(move || {
        x.push("_sub_c");
        v.push(c.get())
    });
    v.expect([8]);
    x.expect(["_sub_c", "c"]);

    let _sub_d = Subscription::computed(move || {
        x.push("_sub_d");
        v.push(d.get())
    });
    v.expect([2]);
    x.expect(["_sub_d", "d"]);

    a.replace_blocking(4);
    v.expect([7, 1]);
    x.expect(["c", "d", "_sub_c", "_sub_d"]);
}
