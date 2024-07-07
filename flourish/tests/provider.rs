use flourish::{shadow_clone, Effect, Provider, SourcePin as _};
mod _validator;
use _validator::Validator;

#[test]
fn cyclic() {
    let v = &Validator::new();

    let p = Provider::new_cyclic((), |weak_provider| {
        move |status| {
            v.push((weak_provider.upgrade().is_some(), status));
        }
    });

    let e = Effect::new(
        {
            shadow_clone!(p);
            move || p.get()
        },
        drop,
    );
    v.expect([(true, true)]);

    drop(p);
    v.expect([]);

    drop(e);
    v.expect([(true, false)]);
}
