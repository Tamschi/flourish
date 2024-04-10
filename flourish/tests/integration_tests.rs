use flourish::{
    raw::RawSubject, shadow_clone, signal, subscription, Signal, Subject, Subscription,
};

#[test]
fn use_constructors() {
    let a = Subject::new(1);
    let b = Subject::new(2);
    let c = Signal::new({
        shadow_clone!(a, b);
        move || a.get() + b.get()
    });
    let d = Signal::new({
        shadow_clone!(a, b);
        move || a.get() - b.get()
    });
    let aa = Signal::new({
        shadow_clone!(c, d);
        move || c.get() + d.get()
    }); //TODO: Make this a cacheless signal.
    let sub = Subscription::new(move || println!("{}", aa.get())); // 2
    b.set_blocking(2); // 2
    a.set_blocking(0); // 0
    drop(sub);

    // These evaluate *no* closures!
    a.set_blocking(2);
    b.set_blocking(3);
    a.set_blocking(5);

    let _sub_c = Subscription::new(move || println!("{}", c.get())); // 8
    let _sub_d = Subscription::new(move || println!("{}", d.get())); // 2
    a.set_blocking(4); // 7, then 1
}

#[test]
fn use_macros() {
    let a = RawSubject::new(1);
    let b = RawSubject::new(2);
    signal! {
        let c => a.get() + b.get();
        let d => a.get() - b.get();
        let aa => c.get() + d.get(); //TODO: Make this a cacheless signal.
    }

    {
        subscription! {
            let sub => println!("{}", aa.get()); // 2
        }
        b.set_blocking(2); // 2
        a.set_blocking(0); // 0
    } // drop sub

    // These evaluate *no* closures!
    a.set_blocking(2);
    b.set_blocking(3);
    a.set_blocking(5);

    subscription! {
        let sub_c => println!("{}", c.get()); // 8
        let sub_d => println!("{}", d.get()); // 2
    }
    a.set_blocking(4); // 7, then 1
}
