#![cfg(feature = "global_signals_runtime")]

use flourish::{shadow_clone, GlobalSignalsRuntime};

type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;

mod _validator;
use _validator::Validator;

#[test]
fn auto_dependencies() {
	let v = &Validator::new();

	let a = Signal::cell("a");
	let b = Signal::cell("b");
	let c = Signal::cell("c");
	let d = Signal::cell("d");
	let e = Signal::cell("e");
	let f = Signal::cell("f");
	let g = Signal::cell("g");
	let index = Signal::cell(0);

	let signal = Signal::computed({
		shadow_clone!(a, b, c, d, e, f, g, index);
		move || {
			v.push(match index.get() {
				1 => a.get(),
				2 => b.get(),
				3 => c.get(),
				4 => d.get(),
				5 => e.get(),
				6 => f.get(),
				7 => g.get(),
				_ => "",
			})
		}
	});
	v.expect([]);

	let subscription = signal.to_subscription();
	v.expect([""]);

	a.set_blocking("a");
	b.set_blocking("b");
	v.expect([]);

	index.set_blocking(1);
	v.expect(["a"]);

	a.set_blocking("aa");
	v.expect(["aa"]);

	b.set_blocking("bb");
	v.expect([]);

	index.set_blocking(2);
	v.expect(["bb"]);

	a.set_blocking("a");
	v.expect([]);

	b.set_blocking("b");
	v.expect(["b"]);

	drop(subscription);
	index.set_blocking(3);
	v.expect([]);
}
