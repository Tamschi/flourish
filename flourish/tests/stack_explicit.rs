use flourish::raw::Source;
mod _validator;
use _validator::Validator;

#[test]
fn use_macros() {
	let v = &Validator::new();
	let x = &Validator::new();

	let a = ::core::pin::pin!(flourish::raw::source_cell(1, flourish::GlobalSignalRuntime));
	let a = ::core::pin::Pin::into_ref(a);
	let b = ::core::pin::pin!(flourish::raw::source_cell(2, flourish::GlobalSignalRuntime));
	let b = ::core::pin::Pin::into_ref(b);
	let (b, set_b) =
		b.as_getter_and_setter(|s| move || s.get(), |s| move |v| s.replace_blocking(v));
	let c = ::core::pin::pin!(flourish::raw::computed(
		|| {
			x.push("c");
			a.get() + b()
		},
		flourish::GlobalSignalRuntime
	));
	let c = ::core::pin::Pin::into_ref(c) as ::core::pin::Pin<&dyn Source<_, Output = _>>;
	let d = ::core::pin::pin!(flourish::raw::computed(
		|| {
			x.push("d");
			a.get() - b()
		},
		flourish::GlobalSignalRuntime
	));
	let d = ::core::pin::Pin::into_ref(d) as ::core::pin::Pin<&dyn Source<_, Output = _>>;
	let aa = ::core::pin::pin!(flourish::raw::computed_uncached(
		|| {
			x.push("aa");
			c.get() + d.get()
		},
		flourish::GlobalSignalRuntime
	));
	let aa = ::core::pin::Pin::into_ref(aa) as ::core::pin::Pin<&dyn Source<_, Output = _>>;
	v.expect([]);
	x.expect([]);

	{
		let _sub_aa = ::core::pin::pin!(flourish::__::new_raw_unsubscribed_subscription(
			flourish::raw::computed(
				|| {
					x.push("sub_aa");
					v.push(aa.get())
				},
				flourish::GlobalSignalRuntime
			)
		));
		let _sub_aa = ::core::pin::Pin::into_ref(_sub_aa);
		flourish::__::pull_subscription(_sub_aa);
		let _sub_aa = flourish::__::pin_into_pin_impl_source(_sub_aa);
		v.expect([2]);
		x.expect(["sub_aa", "aa", "c", "d"]);

		set_b(2);
		v.expect([2]);
		x.expect(["c", "d", "sub_aa", "aa"]);

		a.replace_blocking(0);
		v.expect([0]);
		x.expect(["c", "d", "sub_aa", "aa"]);
	} // drop sub

	// These evaluate *no* closures!
	a.replace_blocking(2);
	set_b(3);
	a.replace_blocking(5);
	v.expect([]);
	x.expect([]);

	let _sub_c = ::core::pin::pin!(flourish::__::new_raw_unsubscribed_subscription(
		flourish::raw::computed(
			|| {
				x.push("sub_c");
				v.push(c.get())
			},
			flourish::GlobalSignalRuntime
		)
	));
	let _sub_c = ::core::pin::Pin::into_ref(_sub_c);
	flourish::__::pull_subscription(_sub_c);
	let _sub_c = flourish::__::pin_into_pin_impl_source(_sub_c);
	let _sub_d = ::core::pin::pin!(flourish::__::new_raw_unsubscribed_subscription(
		flourish::raw::computed(
			|| {
				x.push("sub_d");
				v.push(d.get())
			},
			flourish::GlobalSignalRuntime
		)
	));
	let _sub_d = ::core::pin::Pin::into_ref(_sub_d);
	flourish::__::pull_subscription(_sub_d);
	let _sub_d = flourish::__::pin_into_pin_impl_source(_sub_d);
	v.expect([8, 2]);
	x.expect(["sub_c", "c", "sub_d", "d"]);

	a.replace_blocking(4);
	v.expect([7, 1]);
	x.expect(["c", "d", "sub_c", "sub_d"]);
}
