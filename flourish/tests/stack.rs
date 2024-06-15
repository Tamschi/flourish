use flourish::{signal, subject, subscription};
mod _validator;
use _validator::Validator;

#[test]
fn use_macros() {
    let v = &Validator::new();

    subject! {
        let a := 1;
        let b := 2;
    }
    let (b, set_b) = b.get_set();
    signal! {
        let c => a.get() + b();
        let d => a.get() - b();
        let aa => c.get() + d.get(); //TODO: Make this a cacheless signal.
    }
    v.expect([]);

    {
        subscription! {
            let sub => v.push(aa.get());
        }
        v.expect([2]);

        set_b(2);
        v.expect([2]);

        a.set(0);
        v.expect([0]);
    } // drop sub

    // These evaluate *no* closures!
    a.set(2);
    set_b(3);
    a.set(5);
    v.expect([]);

    subscription! {
        let sub_c => v.push(c.get());
        let sub_d => v.push(d.get());
    }
    v.expect([8, 2]);

    a.set(4);
    v.expect([7, 1]);
}
