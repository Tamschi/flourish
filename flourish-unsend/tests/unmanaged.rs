#![cfg(feature = "local_signals_runtime")]

use ::core::pin::{pin, Pin};
use flourish_unsend::{
	unmanaged::{inert_cell, UnmanagedSignal, UnmanagedSignalCell},
	LocalSignalsRuntime,
};
mod _validator;
use _validator::Validator;

#[test]
fn use_unmanaged() {
	let v = &Validator::new();
	let x = &Validator::new();

	let a = pin!(inert_cell(1, LocalSignalsRuntime));
	let a = Pin::into_ref(a);
	let b = pin!(inert_cell(2, LocalSignalsRuntime));
	let b = Pin::into_ref(b);
	let (b, b_cell) = b.as_source_and_cell();
	let c = pin!(flourish_unsend::unmanaged::computed(
		|| {
			x.push("c");
			a.get() + b.get()
		},
		LocalSignalsRuntime
	));
	let c = Pin::into_ref(c) as Pin<&dyn UnmanagedSignal<_, _>>;
	let d = pin!(flourish_unsend::unmanaged::computed(
		|| {
			x.push("d");
			a.get() - b.get()
		},
		LocalSignalsRuntime
	));
	let d = Pin::into_ref(d) as Pin<&dyn UnmanagedSignal<_, _>>;
	let aa = pin!(flourish_unsend::unmanaged::computed_uncached(
		|| {
			x.push("aa");
			c.get() + d.get()
		},
		LocalSignalsRuntime
	));
	let aa = Pin::into_ref(aa) as Pin<&dyn UnmanagedSignal<_, _>>;
	v.expect([]);
	x.expect([]);

	{
		let _sub_aa = pin!(flourish_unsend::__::new_raw_unsubscribed_subscription(
			flourish_unsend::unmanaged::computed(
				|| {
					x.push("sub_aa");
					v.push(aa.get())
				},
				LocalSignalsRuntime
			)
		));
		let _sub_aa = Pin::into_ref(_sub_aa);
		flourish_unsend::__::pull_new_subscription(_sub_aa);
		let _sub_aa = flourish_unsend::__::pin_into_pin_impl_source(_sub_aa);
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

	let _sub_c = pin!(flourish_unsend::__::new_raw_unsubscribed_subscription(
		flourish_unsend::unmanaged::computed(
			|| {
				x.push("sub_c");
				v.push(c.get())
			},
			LocalSignalsRuntime
		)
	));
	let _sub_c = Pin::into_ref(_sub_c);
	flourish_unsend::__::pull_new_subscription(_sub_c);
	let _sub_c = flourish_unsend::__::pin_into_pin_impl_source(_sub_c);
	let _sub_d = pin!(flourish_unsend::__::new_raw_unsubscribed_subscription(
		flourish_unsend::unmanaged::computed(
			|| {
				x.push("sub_d");
				v.push(d.get())
			},
			LocalSignalsRuntime
		)
	));
	let _sub_d = Pin::into_ref(_sub_d);
	flourish_unsend::__::pull_new_subscription(_sub_d);
	let _sub_d = flourish_unsend::__::pin_into_pin_impl_source(_sub_d);
	v.expect([8, 2]);
	x.expect(["sub_c", "c", "sub_d", "d"]);

	a.replace_blocking(4);
	v.expect([7, 1]);
	x.expect(["c", "d", "sub_c", "sub_d"]);
}
