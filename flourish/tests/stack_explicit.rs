#![cfg(feature = "global_signals_runtime")]

use ::core::pin::{pin, Pin};
use flourish::{
	raw::{inert_cell, Source, SourceCell},
	GlobalSignalsRuntime,
};
mod _validator;
use _validator::Validator;

#[test]
fn use_macros() {
	let v = &Validator::new();
	let x = &Validator::new();

	let a = pin!(inert_cell(1, GlobalSignalsRuntime));
	let a = Pin::into_ref(a);
	let b = pin!(inert_cell(2, GlobalSignalsRuntime));
	let b = Pin::into_ref(b);
	let (b, b_cell) = b.as_source_and_cell();
	let c = pin!(flourish::raw::computed(
		|| {
			x.push("c");
			a.get() + b.get()
		},
		GlobalSignalsRuntime
	));
	let c = Pin::into_ref(c) as Pin<&dyn Source<_, Output = _>>;
	let d = pin!(flourish::raw::computed(
		|| {
			x.push("d");
			a.get() - b.get()
		},
		GlobalSignalsRuntime
	));
	let d = Pin::into_ref(d) as Pin<&dyn Source<_, Output = _>>;
	let aa = pin!(flourish::raw::computed_uncached(
		|| {
			x.push("aa");
			c.get() + d.get()
		},
		GlobalSignalsRuntime
	));
	let aa = Pin::into_ref(aa) as Pin<&dyn Source<_, Output = _>>;
	v.expect([]);
	x.expect([]);

	{
		let _sub_aa = pin!(flourish::__::new_raw_unsubscribed_subscription(
			flourish::raw::computed(
				|| {
					x.push("sub_aa");
					v.push(aa.get())
				},
				GlobalSignalsRuntime
			)
		));
		let _sub_aa = Pin::into_ref(_sub_aa);
		flourish::__::pull_subscription(_sub_aa);
		let _sub_aa = flourish::__::pin_into_pin_impl_source(_sub_aa);
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

	let _sub_c = pin!(flourish::__::new_raw_unsubscribed_subscription(
		flourish::raw::computed(
			|| {
				x.push("sub_c");
				v.push(c.get())
			},
			GlobalSignalsRuntime
		)
	));
	let _sub_c = Pin::into_ref(_sub_c);
	flourish::__::pull_subscription(_sub_c);
	let _sub_c = flourish::__::pin_into_pin_impl_source(_sub_c);
	let _sub_d = pin!(flourish::__::new_raw_unsubscribed_subscription(
		flourish::raw::computed(
			|| {
				x.push("sub_d");
				v.push(d.get())
			},
			GlobalSignalsRuntime
		)
	));
	let _sub_d = Pin::into_ref(_sub_d);
	flourish::__::pull_subscription(_sub_d);
	let _sub_d = flourish::__::pin_into_pin_impl_source(_sub_d);
	v.expect([8, 2]);
	x.expect(["sub_c", "c", "sub_d", "d"]);

	a.replace_blocking(4);
	v.expect([7, 1]);
	x.expect(["c", "d", "sub_c", "sub_d"]);
}
