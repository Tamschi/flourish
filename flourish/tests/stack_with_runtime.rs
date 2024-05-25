use flourish::{signal, subject, subscription};
use pollinate::runtime::GlobalSignalRuntime;

#[test]
fn use_macros() {
    subject! {GlobalSignalRuntime=>
        let a := 1;
        let b := 2;
    }
    let (b, set_b) = b.get_set();
    signal! {GlobalSignalRuntime=>
        let c => a.get() + b();
        let d => a.get() - b();
        let aa => c.get() + d.get(); //TODO: Make this a cacheless signal.
    }

    {
        subscription! {GlobalSignalRuntime=>
            let sub => println!("{}", aa.get()); // 2
        }
        set_b(2); // 2
        a.set(0); // 0
    } // drop sub

    // These evaluate *no* closures!
    a.set(2);
    set_b(3);
    a.set(5);

    subscription! {GlobalSignalRuntime=>
        let sub_c => println!("{}", c.get()); // 8
        let sub_d => println!("{}", d.get()); // 2
    }
    a.set(4); // 7, then 1
}
