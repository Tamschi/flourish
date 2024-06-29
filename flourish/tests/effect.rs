use flourish::{Effect, Subject};
mod _validator;
use _validator::Validator;

#[test]
fn heap() {
    let v = &Validator::new();

    let (a, set_a) = Subject::new(()).into_get_set();

    let e = Effect::new(
        move || {
            a();
            v.push("f")
        },
        |()| v.push("drop"),
    );
    v.expect(["f"]);

    set_a(());
    v.expect(["drop", "f"]);

    drop(e);
    v.expect(["drop"]);

    drop(set_a);
    v.expect([]);
}
