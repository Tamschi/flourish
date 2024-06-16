use flourish::{signal, subject, subscription};
mod _validator;
use _validator::Validator;

#[test]
fn use_macros() {
    let v = &Validator::new();
    let x = &Validator::new();

    subject! {
        let a := 1;
        let b := 2;
    }
    let (b, set_b) = b.get_set();
    signal! {
        let c => { x.push("c"); a.get() + b() };
        let d => { x.push("d"); a.get() - b() };
        let aa =>{ x.push("aa");  c.get() + d.get() }; //TODO: Make this a cacheless signal.
    }
    v.expect([]);
    x.expect([]);

    {
        subscription! {
            let sub_aa => { x.push("sub_aa"); v.push(aa.get()) };
        }
        v.expect([2]);
        x.expect(["sub_aa", "aa", "c", "d"]);

        set_b(2);
        v.expect([2]);
        x.expect(["c", "d", "aa", "sub_aa"]);

        a.set(0);
        v.expect([0]);
        x.expect(["c", "d", "aa", "sub_aa"]);
    } // drop sub

    // These evaluate *no* closures!
    a.set(2);
    set_b(3);
    a.set(5);
    v.expect([]);
    x.expect([]);

    subscription! {
        let sub_c => { x.push("sub_c"); v.push(c.get()) };
        let sub_d => { x.push("sub_d"); v.push(d.get()) };
    }
    v.expect([8, 2]);
    x.expect(["sub_c", "c", "sub_d", "d"]);

    a.set(4);
    v.expect([7, 1]);
    x.expect(["c", "d", "sub_c", "sub_d"]);
}
