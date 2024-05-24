use flourish::{shadow_clone, Signal, Subject, Subscription};

#[test]
fn use_constructors() {
    let a = Subject::<_>::new(1);
    let (b, set_b) = Subject::<_>::new(2).into_get_set();
    let c = Signal::<_>::new({
        shadow_clone!(a, b);
        move || a.get() + b()
    });
    let d = Signal::<_>::new({
        shadow_clone!(a, b);
        move || a.get() - b()
    });
    let aa = Signal::<_>::new({
        shadow_clone!(c, d);
        move || c.get() + d.get()
    }); //TODO: Make this a cacheless signal.
    let sub = Subscription::<_>::new(move || println!("{}", aa.get())); // 2
    set_b(2); // 2
    a.set(0); // 0
    drop(sub);

    // These evaluate *no* closures!
    a.set(2);
    set_b(3);
    a.set(5);

    let _sub_c = Subscription::<_>::new(move || println!("{}", c.get())); // 8
    let _sub_d = Subscription::<_>::new(move || println!("{}", d.get())); // 2
    a.set(4); // 7, then 1
}
