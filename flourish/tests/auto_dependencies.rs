use flourish::{prelude::*, shadow_clone, Signal, SignalCell, Subscription};

mod _validator;
use _validator::Validator;

#[test]
fn auto_dependencies() {
	let v = &Validator::new();

	let a = SignalCell::new("a");
	let b = SignalCell::new("b");
	let c = SignalCell::new("c");
	let d = SignalCell::new("d");
	let e = SignalCell::new("e");
	let f = SignalCell::new("f");
	let g = SignalCell::new("g");
	let index = SignalCell::new(0);

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

	let subscription = Subscription::computed(|| signal.touch());
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
