use ::core::pin::{pin, Pin};

use flourish::{
    raw::{computed, computed_uncached, subject},
    GlobalSignalRuntime, Source, SubscribableSource,
    __::{new_raw_unsubscribed_subscription, pin_into_pin_impl_source},
};
mod _validator;
use _validator::Validator;

#[test]
fn use_macros() {
    let v = &Validator::new();
    let x = &Validator::new();

    let a = pin!(subject(1, GlobalSignalRuntime));
    let a = Pin::into_ref(a);
    let b = pin!(subject(2, GlobalSignalRuntime));
    let b = Pin::into_ref(b);
    let (b, set_b) = b.get_set();
    let c = pin!(computed(
        || {
            x.push("c");
            a.get() + b()
        },
        GlobalSignalRuntime
    ));
    let c = SubscribableSource::ref_as_source(Pin::into_ref(c));
    let d = pin!(computed(
        || {
            x.push("d");
            a.get() - b()
        },
        GlobalSignalRuntime
    ));
    let d = SubscribableSource::ref_as_source(Pin::into_ref(d));
    let aa = pin!(computed_uncached(
        || {
            x.push("aa");
            c.get() + d.get()
        },
        GlobalSignalRuntime
    ));
    let aa = SubscribableSource::ref_as_source(Pin::into_ref(aa));
    v.expect([]);
    x.expect([]);

    {
        let _sub_aa = pin!(new_raw_unsubscribed_subscription(computed(
            || {
                x.push("sub_aa");
                v.push(aa.get())
            },
            GlobalSignalRuntime
        )));
        let _sub_aa = Pin::into_ref(_sub_aa);
        flourish::__::pull_subscription(_sub_aa);
        let _sub_aa = pin_into_pin_impl_source(_sub_aa);
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

    let _sub_c = pin!(new_raw_unsubscribed_subscription(computed(
        || {
            x.push("sub_c");
            v.push(c.get())
        },
        GlobalSignalRuntime
    )));
    let _sub_c = Pin::into_ref(_sub_c);
    flourish::__::pull_subscription(_sub_c);
    let _sub_c = pin_into_pin_impl_source(_sub_c);
    let _sub_d = pin!(new_raw_unsubscribed_subscription(computed(
        || {
            x.push("sub_d");
            v.push(d.get())
        },
        GlobalSignalRuntime
    )));
    let _sub_d = Pin::into_ref(_sub_d);
    flourish::__::pull_subscription(_sub_d);
    let _sub_d = pin_into_pin_impl_source(_sub_d);
    v.expect([8, 2]);
    x.expect(["sub_c", "c", "sub_d", "d"]);

    a.set(4);
    v.expect([7, 1]);
    x.expect(["c", "d", "sub_c", "sub_d"]);
}
