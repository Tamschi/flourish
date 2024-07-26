#![cfg(feature = "global_signals_runtime")]

use flourish::{
	signals_helper,
	unmanaged::{UnmanagedSignal, UnmanagedSignalCell},
};
mod _validator;
use _validator::Validator;

#[test]
fn stack() {
	let v = &Validator::new();

	{
		signals_helper! {
			let a = inert_cell!(());
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

			a.replace_blocking(());
			v.expect(["drop", "f"]);
		} // drop e
		v.expect(["drop"]);
	}
	v.expect([]);
}
