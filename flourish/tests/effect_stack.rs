use flourish::signals_helper;
mod _validator;
use _validator::Validator;

#[test]
fn stack() {
    let v = &Validator::new();

    {
        signals_helper! {
            let a = subject!(());
        }
        {
            signals_helper! {
                let _e = effect!(
                    move || {
                        a.get();
                        v.push("f")
                    },
                    |()| v.push("drop"),
                );
            }
            v.expect(["f"]);

            a.set(());
            v.expect(["drop", "f"]);
        } // drop e
        v.expect(["drop"]);
    }
    v.expect([]);
}
