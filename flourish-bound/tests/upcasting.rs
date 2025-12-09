#![cfg(feature = "global_signals_runtime")]

use flourish_bound::GlobalSignalsRuntime;
mod _validator;

type Signal<T, S> = flourish_bound::Signal<T, S, GlobalSignalsRuntime>;
type SignalArcDyn<'a, T> = flourish_bound::SignalArcDyn<'a, T, GlobalSignalsRuntime>;
type SignalArcDynCell<'a, T> = flourish_bound::SignalArcDynCell<'a, T, GlobalSignalsRuntime>;
type SignalWeakDyn<'a, T> = flourish_bound::SignalWeakDyn<'a, T, GlobalSignalsRuntime>;
type SignalWeakDynCell<'a, T> = flourish_bound::SignalWeakDynCell<'a, T, GlobalSignalsRuntime>;
type SubscriptionDyn<'a, T> = flourish_bound::SubscriptionDyn<'a, T, GlobalSignalsRuntime>;
type SubscriptionDynCell<'a, T> = flourish_bound::SubscriptionDynCell<'a, T, GlobalSignalsRuntime>;

#[test]
fn methods() {
	let arc = Signal::cell(0).into_dyn_cell();
	let weak = arc.downgrade();
	let sub = arc.to_subscription();

	// `Signal` methods.
	let _ = arc.as_read_only();
	let _ = arc.to_read_only();

	// Handle methods.
	let _ = arc.into_read_only_and_self().1.into_read_only();
	let _ = weak.into_read_only_and_self().1.into_read_only();
	let _ = sub.into_read_only();
}

#[test]
fn via_into() {
	// Unsizing.
	let arc: SignalArcDynCell<_> = Signal::cell(0).into();
	let weak: SignalWeakDynCell<_> = Signal::cell(0).downgrade().into();
	let sub: SubscriptionDynCell<_> = Signal::cell(0).to_subscription().into();

	let _: SignalArcDyn<_> = arc.into();
	let _: SignalWeakDyn<_> = weak.into();
	let _: SubscriptionDyn<_> = sub.into();
}
