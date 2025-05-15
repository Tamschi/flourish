#![cfg(feature = "global_signals_runtime")]

use flourish::{GlobalSignalsRuntime, Propagation};

type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;

#[test]
fn eager() {
	let cell = Signal::cell(());

	let a = cell.set_eager(());
	let b = cell.set_eager_dyn(());
	let c = cell.update_eager(|_| (Propagation::Halt, ()));
	let d = cell.update_eager_dyn(Box::new(|_| Propagation::Halt));
	let e = cell.replace_eager(());
	let f = cell.replace_eager_dyn(());
	let g = cell.set_distinct_eager(());
	let h = cell.set_distinct_eager_dyn(());
	let i = cell.replace_distinct_eager(());
	let j = cell.replace_distinct_eager_dyn(());

	drop(cell);

	drop((a, b, c, d, e, f, g, h, i, j));
}

#[test]
fn r#async() {
	let cell = Signal::cell(());

	let a = cell.set_async(());
	let b = cell.set_async_dyn(());
	let c = cell.update_async(|_| (Propagation::Halt, ()));
	let d = cell.update_async_dyn(Box::new(|_| Propagation::Halt));
	let e = cell.replace_async(());
	let f = cell.replace_async_dyn(());
	let g = cell.set_distinct_async(());
	let h = cell.set_distinct_async_dyn(());
	let i = cell.replace_distinct_async(());
	let j = cell.replace_distinct_async_dyn(());

	drop(cell);

	drop((a, b, c, d, e, f, g, h, i, j));
}
