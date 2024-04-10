use flourish::{signal, subscription};

#[test]
fn use_macros() {
    signal! { let signal => (); }
    subscription! { let subscription => signal.get(); }
}
