#![cfg(feature = "global_signals_runtime")]

use flourish::GlobalSignalsRuntime;

type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
type Subscription<T, S> = flourish::Subscription<T, S, GlobalSignalsRuntime>;

mod _validator;
use _validator::Validator;

#[test]
fn debounce_test() {
	let v = &Validator::new();
	let x = &Validator::new();

	let (signal, cell) = Signal::cell(0).into_read_only_and_self_dyn();
	let debounced = Signal::debounced(move || {
		x.push("d");
		signal.get()
	});
	let _sub = Subscription::computed(move || {
		x.push("s");
		v.push(debounced.get())
	});
	v.expect([0]);
	x.expect(["s", "d"]);

	let mut previous = 0;
	for n in [1, 2, 3, 3, 4, 5, 5, 5, 6, 6, 6, 7, 7, 7, 7, 8, 9, 9] {
		cell.replace_blocking(n);
		if n == previous {
			x.expect(["d"]);
		} else {
			x.expect(["d", "s"]);
		}
		previous = n;
	}
	v.expect([1, 2, 3, 4, 5, 6, 7, 8, 9]);
}
