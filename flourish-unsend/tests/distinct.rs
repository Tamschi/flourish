#![cfg(feature = "local_signals_runtime")]

use flourish_unsend::LocalSignalsRuntime;

type Signal<T, S> = flourish_unsend::Signal<T, S, LocalSignalsRuntime>;
type Subscription<T, S> = flourish_unsend::Subscription<T, S, LocalSignalsRuntime>;

mod _validator;
use _validator::Validator;

#[test]
fn distinct() {
	let v = &Validator::new();
	let x = &Validator::new();

	let (signal, cell) = Signal::cell(0).into_dyn_read_only_and_self();
	let distinct = Signal::distinct(move || {
		x.push("d");
		signal.get()
	});
	let _sub = Subscription::computed(move || {
		x.push("s");
		v.push(distinct.get())
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
