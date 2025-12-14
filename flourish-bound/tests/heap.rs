#![cfg(feature = "local_signals_runtime")]

use flourish_bound::{shadow_clone, LocalSignalsRuntime};

type Signal<T, S> = flourish_bound::Signal<T, S, LocalSignalsRuntime>;
type Subscription<T, S> = flourish_bound::Subscription<T, S, LocalSignalsRuntime>;

mod _validator;
use _validator::Validator;

#[test]
fn use_constructors() {
	let v = &Validator::new();
	let x = &Validator::new();

	let a = Signal::cell(1);
	let (b, b_cell) = Signal::cell(2).into_dyn_read_only_and_self();
	let c = Signal::computed({
		shadow_clone!(a, b);
		move || {
			x.push("c");
			a.get() + b.get()
		}
	});
	let d = Signal::computed({
		shadow_clone!(a, b);
		move || {
			x.push("d");
			a.get() - b.get()
		}
	});
	let aa = Signal::computed_uncached({
		shadow_clone!(c, d);
		move || {
			x.push("aa");
			c.get() + d.get()
		}
	});
	v.expect([]);
	x.expect([]);

	let sub_aa = Subscription::computed(move || {
		x.push("sub_aa");
		v.push(aa.get())
	});
	v.expect([2]);
	x.expect(["sub_aa", "aa", "c", "d"]);

	b_cell.replace_blocking(2);
	v.expect([2]);
	x.expect(["c", "d", "sub_aa", "aa"]);

	a.replace_blocking(0);
	v.expect([0]);
	x.expect(["c", "d", "sub_aa", "aa"]);

	drop(sub_aa);

	// These evaluate *no* closures!
	a.replace_blocking(2);
	b_cell.replace_blocking(3);
	a.replace_blocking(5);
	v.expect([]);
	x.expect([]);

	let _sub_c = Subscription::computed(move || {
		x.push("_sub_c");
		v.push(c.get())
	});
	v.expect([8]);
	x.expect(["_sub_c", "c"]);

	let _sub_d = Subscription::computed(move || {
		x.push("_sub_d");
		v.push(d.get())
	});
	v.expect([2]);
	x.expect(["_sub_d", "d"]);

	a.replace_blocking(4);
	v.expect([7, 1]);
	x.expect(["c", "d", "_sub_c", "_sub_d"]);
}
