use flourish::{signals_helper, GlobalSignalRuntime, Source};
mod _validator;
use _validator::Validator;

#[test]
fn use_macros() {
    let v = &Validator::new();
    let x = &Validator::new();

    signals_helper! {
        let a = subject_sr!(1, GlobalSignalRuntime);
        let b = subject_sr!(2, GlobalSignalRuntime);
    }
    let (b, set_b) = b.get_set();
    signals_helper! {
        let c = computed_sr!(|| {
            x.push("c");
            a.get() + b()
        }, GlobalSignalRuntime);
        let d = computed_sr!(|| {
            x.push("d");
            a.get() - b()
        }, GlobalSignalRuntime);
        let aa = uncached_sr!(|| {
            x.push("aa");
            c.get() + d.get()
        }, GlobalSignalRuntime);
    }
    v.expect([]);
    x.expect([]);

    {
        signals_helper! {
            let _sub_aa = subscription_sr!(|| { x.push("sub_aa"); v.push(aa.get()) }, GlobalSignalRuntime);
        }
        v.expect([2]);
        x.expect(["sub_aa", "aa", "c", "d"]);

        set_b(2);
        v.expect([2]);
        x.expect(["c", "d", "sub_aa", "aa"]);

        a.set(0);
        v.expect([0]);
        x.expect(["c", "d", "sub_aa", "aa"]);
    } // drop sub

    // These evaluate *no* closures!
    a.set(2);
    set_b(3);
    a.set(5);
    v.expect([]);
    x.expect([]);

    signals_helper! {
        let _sub_c = subscription_sr!(|| { x.push("sub_c"); v.push(c.get()) }, GlobalSignalRuntime);
        let _sub_d = subscription_sr!(|| { x.push("sub_d"); v.push(d.get()) }, GlobalSignalRuntime);
    }
    v.expect([8, 2]);
    x.expect(["sub_c", "c", "sub_d", "d"]);

    a.set(4);
    v.expect([7, 1]);
    x.expect(["c", "d", "sub_c", "sub_d"]);
}
