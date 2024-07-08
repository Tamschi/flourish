use std::sync::Mutex;

use flourish::{Signal, SignalCell, SourcePin as _, Subscription};
mod _validator;
use _validator::Validator;

//FIXME: This has a race condition somewhere!

#[test]
fn heap() {
	let v = &Validator::new();

	let (a, set_a) = SignalCell::new(())
		.into_getter_and_setter(|s| move || s.get(), |s| move |v| s.replace_blocking(v));
	let (b, set_b) = SignalCell::new(())
		.into_getter_and_setter(|s| move || s.get(), |s| move |v| s.replace_blocking(v));
	let (c, set_c) = SignalCell::new(())
		.into_getter_and_setter(|s| move || s.get(), |s| move |v| s.replace_blocking(v));

	let roundabout = Signal::computed_uncached_mut({
		let mut angle = 0;
		move || {
			match angle {
				0 => a(),
				1 => b(),
				2 => c(),
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

	set_b(());
	v.expect(['a', 'b']);

	set_b(());
	set_c(());
	v.expect([]);

	set_a(());
	v.expect(['a', 'b']);
}

#[test]
fn stack() {
	let v = &Validator::new();

	let (a, set_a) = SignalCell::new(())
		.into_getter_and_setter(|s| move || s.get(), |s| move |v| s.replace_blocking(v));
	let (b, set_b) = SignalCell::new(())
		.into_getter_and_setter(|s| move || s.get(), |s| move |v| s.replace_blocking(v));
	let (c, set_c) = SignalCell::new(())
		.into_getter_and_setter(|s| move || s.get(), |s| move |v| s.replace_blocking(v));

	let roundabout = Signal::computed_uncached({
		let angle = Mutex::new(0);
		move || {
			let mut angle = angle.lock().unwrap();
			match *angle {
				0 => a(),
				1 => b(),
				2 => c(),
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

	set_b(());
	v.expect(['a', 'b']);

	set_b(());
	set_c(());
	v.expect([]);

	set_a(());
	v.expect(['a', 'b']);
}
