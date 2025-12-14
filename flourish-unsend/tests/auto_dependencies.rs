#![cfg(feature = "local_signals_runtime")]

use flourish_unsend::{shadow_clone, LocalSignalsRuntime};

type Signal<T, S> = flourish_unsend::Signal<T, S, LocalSignalsRuntime>;

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

	a.replace_blocking("a");
	b.replace_blocking("b");
	v.expect([]);

	index.replace_blocking(1);
	v.expect(["a"]);

	a.replace_blocking("aa");
	v.expect(["aa"]);

	b.replace_blocking("bb");
	v.expect([]);

	index.replace_blocking(2);
	v.expect(["bb"]);

	a.replace_blocking("a");
	v.expect([]);

	b.replace_blocking("b");
	v.expect(["b"]);

	drop(subscription);
	index.replace_blocking(3);
	v.expect([]);
}
