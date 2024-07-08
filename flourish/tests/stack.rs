use flourish::{raw::Source, signals_helper};
mod _validator;
use _validator::Validator;

#[test]
fn use_macros() {
    let v = &Validator::new();
    let x = &Validator::new();

    signals_helper! {
        let a = subject!(1);
        let b = subject!(2);
    }
    let (b, set_b) =
        b.to_mapped_source_sender(|s| move || s.get(), |s| move |v| s.replace_blocking(v));
    signals_helper! {
        let c = computed!(|| {
            x.push("c");
            a.get() + b()
        });
        let d = computed!(|| {
            x.push("d");
            a.get() - b()
        });
        let aa = computed_uncached!(|| {
            x.push("aa");
            c.get() + d.get()
        });
    }
    v.expect([]);
    x.expect([]);

    {
        signals_helper! {
            let _sub_aa = subscription!(|| { x.push("sub_aa"); v.push(aa.get()) });
        }
        v.expect([2]);
        x.expect(["sub_aa", "aa", "c", "d"]);

        set_b(2);
        v.expect([2]);
        x.expect(["c", "d", "sub_aa", "aa"]);

        a.replace_blocking(0);
        v.expect([0]);
        x.expect(["c", "d", "sub_aa", "aa"]);
    } // drop sub

    // These evaluate *no* closures!
    a.replace_blocking(2);
    set_b(3);
    a.replace_blocking(5);
    v.expect([]);
    x.expect([]);

    signals_helper! {
        let _sub_c = subscription!(|| { x.push("sub_c"); v.push(c.get()) });
        let _sub_d = subscription!(|| { x.push("sub_d"); v.push(d.get()) });
    }
    v.expect([8, 2]);
    x.expect(["sub_c", "c", "sub_d", "d"]);

    a.replace_blocking(4);
    v.expect([7, 1]);
    x.expect(["c", "d", "sub_c", "sub_d"]);
}
