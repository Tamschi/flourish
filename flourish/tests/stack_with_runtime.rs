#![cfg(feature = "global_signal_runtime")]

use flourish::{
	raw::{Source, SourceCell},
	signals_helper, GlobalSignalRuntime,
};
mod _validator;
use _validator::Validator;

#[test]
fn use_macros() {
	let v = &Validator::new();
	let x = &Validator::new();

	signals_helper! {
		let a = inert_cell_with_runtime!(1, GlobalSignalRuntime);
		let b = inert_cell_with_runtime!(2, GlobalSignalRuntime);
	}
	let (b, b_cell) = b.as_source_and_cell();
	signals_helper! {
		let c = computed_with_runtime!(|| {
			x.push("c");
			a.get() + b.get()
		}, GlobalSignalRuntime);
		let d = computed_with_runtime!(|| {
			x.push("d");
			a.get() - b.get()
		}, GlobalSignalRuntime);
		let aa = computed_uncached_with_runtime!(|| {
			x.push("aa");
			c.get() + d.get()
		}, GlobalSignalRuntime);
	}
	v.expect([]);
	x.expect([]);

	{
		signals_helper! {
			let _sub_aa = subscription_with_runtime!(|| { x.push("sub_aa"); v.push(aa.get()) }, GlobalSignalRuntime);
		}
		v.expect([2]);
		x.expect(["sub_aa", "aa", "c", "d"]);

		b_cell.replace_blocking(2);
		v.expect([2]);
		x.expect(["c", "d", "sub_aa", "aa"]);

		a.replace_blocking(0);
		v.expect([0]);
		x.expect(["c", "d", "sub_aa", "aa"]);
	} // drop sub

	// These evaluate *no* closures!
	a.replace_blocking(2);
	b_cell.replace_blocking(3);
	a.replace_blocking(5);
	v.expect([]);
	x.expect([]);

	signals_helper! {
		let _sub_c = subscription_with_runtime!(|| { x.push("sub_c"); v.push(c.get()) }, GlobalSignalRuntime);
		let _sub_d = subscription_with_runtime!(|| { x.push("sub_d"); v.push(d.get()) }, GlobalSignalRuntime);
	}
	v.expect([8, 2]);
	x.expect(["sub_c", "c", "sub_d", "d"]);

	a.replace_blocking(4);
	v.expect([7, 1]);
	x.expect(["c", "d", "sub_c", "sub_d"]);
}
