use flourish::{Effect, SourcePin, Announcer};
mod _validator;
use _validator::Validator;

#[test]
fn heap() {
    let v = &Validator::new();

    let (a, set_a) = Announcer::new(())
        .into_mapped_source_sender(|s| move || s.get(), |s| move |v| s.replace_blocking(v));

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
