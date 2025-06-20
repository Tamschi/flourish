#![cfg(feature = "global_signals_runtime")]

use std::sync::Mutex;

use flourish::GlobalSignalsRuntime;

type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
type Subscription<T, S> = flourish::Subscription<T, S, GlobalSignalsRuntime>;

mod _validator;
use _validator::Validator;

//FIXME: This has a race condition somewhere! (Does it still?)

#[test]
fn heap() {
	let v = &Validator::new();

	let (a, a_cell) = Signal::cell(()).into_dyn_read_only_and_self();
	let (b, b_cell) = Signal::cell(()).into_dyn_read_only_and_self();
	let (c, c_cell) = Signal::cell(()).into_dyn_read_only_and_self();

	let roundabout = Signal::computed_uncached_mut({
		let mut angle = 0;
		move || {
			match angle {
				0 => a.get(),
				1 => b.get(),
				2 => c.get(),
				_ => unreachable!(),
			}
			angle = (angle + 1) % 3;
		}
	});
	v.expect([]);

	let _a = Subscription::computed(|| {
		v.push('a');
		roundabout.get()
	});
	v.expect(['a']);
	let _b = Subscription::computed(|| {
		v.push('b');
		roundabout.get()
	});
	v.expect(['b']);

	// There are two subscriptions, so each "hit" advances twice.

	b_cell.replace_blocking(());
	v.expect(['a', 'b']);

	b_cell.replace_blocking(());
	c_cell.replace_blocking(());
	v.expect([]);

	a_cell.replace_blocking(());
	v.expect(['a', 'b']);
}

#[test]
fn stack() {
	let v = &Validator::new();

	let (a, a_cell) = Signal::cell(()).into_dyn_read_only_and_self();
	let (b, b_cell) = Signal::cell(()).into_dyn_read_only_and_self();
	let (c, c_cell) = Signal::cell(()).into_dyn_read_only_and_self();

	let roundabout = Signal::computed_uncached({
		let angle = Mutex::new(0);
		move || {
			let mut angle = angle.lock().unwrap();
			match *angle {
				0 => a.get(),
				1 => b.get(),
				2 => c.get(),
				_ => unreachable!(),
			}
			*angle = (*angle + 1) % 3;
		}
	});
	v.expect([]);

	let _a = Subscription::computed(|| {
		v.push('a');
		roundabout.get()
	});
	v.expect(['a']);
	let _b = Subscription::computed(|| {
		v.push('b');
		roundabout.get()
	});
	v.expect(['b']);

	// There are two subscriptions, so each "hit" advances twice.

	b_cell.replace_blocking(());
	v.expect(['a', 'b']);

	b_cell.replace_blocking(());
	c_cell.replace_blocking(());
	v.expect([]);

	a_cell.replace_blocking(());
	v.expect(['a', 'b']);
}
