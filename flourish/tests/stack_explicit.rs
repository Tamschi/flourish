use flourish::{GlobalSignalRuntime, Source};
mod _validator;
use _validator::Validator;

#[test]
fn use_macros() {
    let v = &Validator::new();
    let x = &Validator::new();

    let a = ::core::pin::pin!(flourish::raw::subject(1));
    let a = a.into_ref();
    let b = ::core::pin::pin!(flourish::raw::subject(2));
    let b = b.into_ref();
    let (b, set_b) = b.get_set();
    let c = ::core::pin::pin!(flourish::raw::computed((
        || {
            x.push("c");
            a.get() + b()
        },
        GlobalSignalRuntime
    )));
    let c = c.into_ref();
    let d = ::core::pin::pin!(flourish::raw::computed((
        || {
            x.push("d");
            a.get() - b()
        },
        GlobalSignalRuntime
    )));
    let d = d.into_ref();
    let aa = ::core::pin::pin!(flourish::raw::uncached((
        || {
            x.push("aa");
            c.get() + d.get()
        },
        GlobalSignalRuntime
    )));
    let aa = aa.into_ref();
    v.expect([]);
    x.expect([]);

    {
        let sub_aa = ::core::pin::pin!(
            flourish::__::new_raw_unsubscribed_subscription_with_runtime(
                || {
                    x.push("sub_aa");
                    v.push(aa.get())
                },
                flourish::GlobalSignalRuntime
            )
        );
        let sub_aa = sub_aa.into_ref();
        flourish::__::pull_subscription(sub_aa);
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

    let sub_c = ::core::pin::pin!(
        flourish::__::new_raw_unsubscribed_subscription_with_runtime(
            || {
                x.push("sub_c");
                v.push(c.get())
            },
            flourish::GlobalSignalRuntime
        )
    );
    let sub_c = sub_c.into_ref();
    flourish::__::pull_subscription(sub_c);
    let sub_d = ::core::pin::pin!(
        flourish::__::new_raw_unsubscribed_subscription_with_runtime(
            || {
                x.push("sub_d");
                v.push(d.get())
            },
            flourish::GlobalSignalRuntime
        )
    );
    let sub_d = sub_d.into_ref();
    flourish::__::pull_subscription(sub_d);
    v.expect([8, 2]);
    x.expect(["sub_c", "c", "sub_d", "d"]);

    a.set(4);
    v.expect([7, 1]);
    x.expect(["c", "d", "sub_c", "sub_d"]);
}
