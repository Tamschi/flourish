#![cfg(feature = "global_signals_runtime")]

#[test]
fn test() {
	use std::sync::atomic::{AtomicI32, Ordering::Relaxed};

	use flourish::{shadow_clone, GlobalSignalsRuntime, Propagation};

	// Choose a runtime: (You should do this once centrally in your app.)
	type Effect<'a> = flourish::Effect<'a, GlobalSignalsRuntime>;
	type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
	type SignalDyn<'a, T> = flourish::SignalDyn<'a, T, GlobalSignalsRuntime>;
	// type SignalDynCell<'a, T> = flourish::SignalDynCell<'a, T, GlobalSignalsRuntime>;
	// type SignalArc<T, S> = flourish::SignalArc<T, S, GlobalSignalsRuntime>;
	type SignalArcDyn<'a, T> = flourish::SignalArcDyn<'a, T, GlobalSignalsRuntime>;
	// type SignalArcDynCell<'a, T> = flourish::SignalArcDynCell<'a, T, GlobalSignalsRuntime>;
	type Subscription<T, S> = flourish::Subscription<T, S, GlobalSignalsRuntime>;
	type SubscriptionDyn<'a, T> = flourish::SubscriptionDyn<'a, T, GlobalSignalsRuntime>;
	// type SubscriptionDynCell<'a, T> = flourish::SubscriptionDynCell<'a, T, GlobalSignalsRuntime>;

	let a = Signal::cell(1);
	let b = Signal::cell(2);

	// Won't run yet, as signals are lazy.
	let sum = Signal::computed({
		shadow_clone!(a, b);
		move || a.get() + b.get()
	});

	// Evaluate on demand:
	assert_eq!(sum.get(), 3);

	// Subscribe. This can notify dependencies and keeps the value fresh automatically.
	// Doesn't rerun the closure here, as the value is already fresh from the `.get()` above.
	let mut sub = sum.to_subscription();

	// You can get the value from subscriptions.
	assert_eq!(sub.get(), 3);

	// Conversion between subscribed and unsubscribed handles is easy and cheap.
	sub = sub.unsubscribe().into_subscription();
	drop(sub); // Dropping a `Subscription` directly unsubscribes too, sometimes more efficiently.

	// Use `Effect`s to set up side-effects with custom cleanup:
	let result = AtomicI32::new(0);
	let set_result = Effect::new(|| result.store(sum.get(), Relaxed), drop);
	assert_eq!(result.load(Relaxed), 3);

	// Don't use blocking setters in callbacks!
	// There are deferrable (without suffix) and cancellable `_async` and `_eager` variants too.
	a.set_distinct_blocking(5); // Replaces only distinct values.
	assert_eq!(result.load(Relaxed), 7);

	drop(set_result);

	// `GlobalSignalsRuntime` guarantees subscribed values are fresh whenever the
	// last call into it exits, so *without concurrency* this is synchronous *here*.
	b.set(0);
	assert_eq!(result.load(Relaxed), 7); // unchanged, as `set_result` was dropped.

	// Erase the closure type, at cost of dynamic dispatch through such handles:
	sum.as_dyn();
	let _ = sum.to_dyn(); // Clones the handle (like `Arc`).
	let sum = sum.into_dyn();

	// Type-erased signals can be stored easily.
	struct LiveSum(SignalArcDyn<'static, i32>);
	let sum = LiveSum(sum).0;

	// Pass by heap reference without indirection…
	let _ = dyn_to_shared(&sum);
	fn dyn_to_shared<'a, T: ?Sized + Send>(signal: &SignalDyn<'a, T>) -> SignalArcDyn<'a, T> {
		if true {
			// …and derive an owned handle:
			signal.to_owned()
		} else {
			// If you always call `.to_owned()`, then it's better to use a `SignalArc` parameter instead.
			unreachable!()
		}
	}

	// You can use this with a subscription too, equally direct:
	let sub = sum.into_subscription();
	let _ = dyn_to_shared(&sub);

	// It's completely fine to access dependencies conditionally:
	let choose_a = Signal::cell(false);
	let chosen = Signal::computed(|| if choose_a.get() { a.get() } else { b.get() });
	let chosen = chosen.into_subscription(); // evaluate

	choose_a.set(true); // changes dependencies, as `chosen` is subscribed
	a.set(10);
	assert_eq!(chosen.get(), 10); // from cache in `chosen`

	// (Just cleanup for the next example.)
	drop(chosen);
	drop(choose_a);
	let sum = sub.unsubscribe();

	// You can loop signal resolution with a deferred setter. This is usually BAD architecture!
	// It can be fine with a custom runtime that resolves e.g. once per game tick.
	let _avoid_0 = Effect::new(
		|| {
			if sum.get() == 0 {
				b.update(|value| {
					*value += 1;
					Propagation::Propagate
				})
			}
		},
		drop,
	);
	a.set_blocking(0);
	b.set_blocking(0);
	assert_eq!(b.get(), 1);

	// Signals can be filtered (but are never empty, so the result is a `Future<Output = Subscription<…>>`):
	let thaw = Signal::cell(false);
	let _freezable = std::pin::pin!(Subscription::filter_mapped(|| thaw.get().then(|| a.get())));

	// You can type-erase that too:
	use std::{
		future::Future,
		pin::{pin, Pin},
	};
	let _freezable_dyn: Pin<&mut dyn Future<Output = SubscriptionDyn<_>>> = pin!(async {
		Subscription::filter_mapped(|| thaw.get().then(|| a.get()))
			.await
			.into_dyn() // `Into` is available for side effect free conversions and combinations thereof.
	});
}
