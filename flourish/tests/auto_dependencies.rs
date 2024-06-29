use flourish::{shadow_clone, Signal, Subject, Subscription};

mod _validator;
use _validator::Validator;

#[test]
fn auto_dependencies() {
    let v = &Validator::new();

    let a = Subject::new("a");
    let b = Subject::new("b");
    let c = Subject::new("c");
    let d = Subject::new("d");
    let e = Subject::new("e");
    let f = Subject::new("f");
    let g = Subject::new("g");
    let index = Subject::new(0);

    let signal = Signal::computed({
        shadow_clone!(a, b, c, d, e, f, g, index);
        move || {
            v.push(match index.get() {
                1 => a.get(),
                2 => b.get(),
                3 => c.get(),
                4 => d.get(),
                5 => e.get(),
                6 => f.get(),
                7 => g.get(),
                _ => "",
            })
        }
    });
    v.expect([]);

    let subscription = Subscription::computed(|| signal.touch());
    v.expect([""]);

    a.set("a");
    b.set("b");
    v.expect([]);

    index.set(1);
    v.expect(["a"]);

    a.set("aa");
    v.expect(["aa"]);

    b.set("bb");
    v.expect([]);

    index.set(2);
    v.expect(["bb"]);

    a.set("a");
    v.expect([]);

    b.set("b");
    v.expect(["b"]);

    drop(subscription);
    index.set(3);
    v.expect([]);
}
