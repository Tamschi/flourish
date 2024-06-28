use std::pin::pin;

use flourish::{
    raw::{computed, computed_uncached, subject},
    signals_helper, GlobalSignalRuntime, Source,
};
mod _validator;
use _validator::Validator;

#[test]
fn use_macros() {
    let v = &Validator::new();
    let x = &Validator::new();

    let a = pin!(subject(1));
    let a = a.into_ref();
    let b = pin!(subject(2));
    let b = b.into_ref();
    let (b, set_b) = b.get_set();
    let c = pin!(computed(
        || {
            x.push("c");
            a.get() + b()
        },
        GlobalSignalRuntime
    ));
    let c = c.into_ref();
    let d = pin!(computed(
        || {
            x.push("d");
            a.get() - b()
        },
        GlobalSignalRuntime
    ));
    let d = d.into_ref();
    let aa = pin!(computed_uncached(
        || {
            x.push("aa");
            c.get() + d.get()
        },
        GlobalSignalRuntime
    ));
    let aa = aa.into_ref();
    v.expect([]);
    x.expect([]);

    {
        let sub_aa = ::core::pin::pin!(flourish::__::new_raw_unsubscribed_subscription(
            flourish::raw::computed(
                (|| {
                    x.push("sub_aa");
                    v.push(aa.get())
                }),
                flourish::GlobalSignalRuntime
            )
        ));
        let sub_aa = ::core::pin::Pin::into_ref(sub_aa);
        flourish::__::pull_subscription(sub_aa);
        let sub_aa = flourish::__::pin_into_pin_impl_source(sub_aa);
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

    let _sub_c = ::core::pin::pin!(flourish::__::new_raw_unsubscribed_subscription(
        flourish::raw::computed(
            (|| {
                x.push("sub_c");
                v.push(c.get())
            }),
            flourish::GlobalSignalRuntime
        )
    ));
    let _sub_c = ::core::pin::Pin::into_ref(_sub_c);
    flourish::__::pull_subscription(_sub_c);
    let _sub_c = flourish::__::pin_into_pin_impl_source(_sub_c);
    let _sub_d = ::core::pin::pin!(flourish::__::new_raw_unsubscribed_subscription(
        flourish::raw::computed(
            (|| {
                x.push("sub_d");
                v.push(d.get())
            }),
            flourish::GlobalSignalRuntime
        )
    ));
    let _sub_d = ::core::pin::Pin::into_ref(_sub_d);
    flourish::__::pull_subscription(_sub_d);
    let _sub_d = flourish::__::pin_into_pin_impl_source(_sub_d);
    v.expect([8, 2]);
    x.expect(["sub_c", "c", "sub_d", "d"]);

    a.set(4);
    v.expect([7, 1]);
    x.expect(["c", "d", "sub_c", "sub_d"]);
}
