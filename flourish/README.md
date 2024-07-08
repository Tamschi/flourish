# `flourish`

Convenient and composable signals for Rust.

🚧 This is a(n optimisable) proof of concept! 🚧  
🚧 The API is full-featured, but the code is not (much at all) optimised. 🚧

Flourish is a signals library inspired by [🚦 JavaScript Signals standard proposal🚦](https://github.com/tc39/proposal-signals?tab=readme-ov-file#-javascript-signals-standard-proposal) (but Rust-y).

When combined with for example [`Option`](https://doc.rust-lang.org/stable/core/option/enum.Option.html) and [`Future`](https://doc.rust-lang.org/stable/core/future/trait.Future.html), `flourish` can model asynchronous-and-cancellable resource loads. See the crate `flourish-extra` for example combinators and `flourish-extensions` to use them conveniently through constructor extensions.

This makes it a suitable replacement for most standard use cases of RxJS-style observables.

## Known Issues

⚠️ The update task queue is currently not fair whatsoever, so one thread looping inside signal processing will block all others.

## Quick-Start

You can put signals on the heap:

```rust
use flourish::{Announcer, Provider, Signal, Update, Subscription, Effect};

let _ = Announcer::new(());
let _ = Provider::new((), |_status| ());
let _ = Provider::new_cyclic((), |_weak| |_status| ());

// The closure type is erased!
// Not evaluated unless subscribed.
let _ = Signal::computed(|| ());
let _ = Signal::computed_uncached(|| ()); // `Fn` closure. The others take `FnMut`s.
let _ = Signal::computed_uncached_mut(|| ());
let _ = Signal::folded((), |_value| Update::Propagate);
let _ = Signal::merged(|| (), |_value, _next| Update::Propagate);

// The closure type is erased!
let _ = Subscription::computed(|| ());
let _ = Subscription::folded((), |_value| Update::Propagate);
let _ = Subscription::merged(|| (), |_value, _next| Update::Propagate);

// The closure and value type are erased!
// Runs `drop` *before* computing the new value.
let _ = Effect::new(|| (), drop);
```

You can also put signals on the stack:

```rust
use flourish::{signals_helper, Update};

signals_helper! {
  let _announcer = announcer!(());
  let _provider = provider!((), |_status| ());

  // The closure type is erased!
  // Not evaluated unless subscribed.
  let _source = computed!(|| ());
  let _source = computed_uncached!(|| ());
  let _source = computed_uncached_mut!(|| ());
  let _source = folded!((), |_value| Update::Propagate);
  let _source = merged!(|| (), |_value, _next| Update::Propagate);

  // The closure type is erased!
  let _source = subscription!(|| ());

  // Runs `drop` *before* computing the new value.
  let _effect = effect!(|| (), drop);
}
```

Additionally, inside `flourish::raw`, you can find constructor functions for unpinned raw signals that enable composition with data-inlining.

## Linking signals

`flourish` detects and updates dependencies automatically:

```rust
use flourish::{shadow_clone, Announcer, Signal, Subscription, SourcePin as _};

let a = Announcer::new("a");
let b = Announcer::new("b");
let c = Announcer::new("c");
let d = Announcer::new("d");
let e = Announcer::new("e");
let f = Announcer::new("f");
let g = Announcer::new("g");
let index = Announcer::new(0);

let signal = Signal::computed({
  shadow_clone!(a, b, c, d, e, f, g, index);
  move || println!("{}", match index.get() {
    1 => a.get(),
    2 => b.get(),
    3 => c.get(),
    4 => d.get(),
    5 => e.get(),
    6 => f.get(),
    7 => g.get(),
    _ => "",
  })
}); // nothing

let subscription = Subscription::computed(|| signal.touch()); // ""

// Note: `change` and `replace` may be deferred (but are safe to use in callbacks)!
//        Use the `…_blocking` and `…_async` variants as needed.
a.replace("a"); b.replace("b"); // nothing
index.change(1); // "a" ("change" methods don't replace or propagate if the value is equal)
a.change("aa"); // "aa"
b.change("bb"); // nothing
index.change(2); // "bb"
a.change("a"); // nothing
b.change("b"); // "b"

drop(subscription);
index.change(3); // nothing
```

`Signal`s are fully lazy, so they only update while subscribed or to refresh their value if dirty.

The default `GlobalSignalRuntime` notifies signals iteratively from earlier to later when possible. Only one such notification cascade is processed at a time with this runtime.

("uncached" signals run their closure whenever their value is retrieved instead, not on update.)

## Using a different runtime

You can use a different [`pollinate`] runtime with the included types and macros (but ideally, alias these items for your own use):

```rust
use flourish::{signals_helper, GlobalSignalRuntime, SignalSR, Announcer, SubscriptionSR, Update};

let _ = Announcer::with_runtime((), GlobalSignalRuntime);

let _ = SignalSR::computed_with_runtime(|| (), GlobalSignalRuntime);
let _ = SignalSR::computed_uncached_with_runtime(|| (), GlobalSignalRuntime);
let _ = SignalSR::computed_uncached_mut_with_runtime(|| (), GlobalSignalRuntime);
let _ = SignalSR::folded_with_runtime((), |_value| Update::Propagate, GlobalSignalRuntime);
let _ = SignalSR::merged_with_runtime(|| (), |_value, _next| Update::Propagate, GlobalSignalRuntime);

let _ = SubscriptionSR::computed_with_runtime(|| (), GlobalSignalRuntime);

signals_helper! {
  let _announcer = announcer_with_runtime!((), GlobalSignalRuntime);

  let _source = computed_with_runtime!(|| (), GlobalSignalRuntime);
  let _source = computed_uncached_with_runtime!(|| (), GlobalSignalRuntime);
  let _source = computed_uncached_mut_with_runtime!(|| (), GlobalSignalRuntime);
  let _source = folded_with_runtime!((), |_value| Update::Propagate, GlobalSignalRuntime);
  let _source = merged_with_runtime!(|| (), |_value, _next| Update::Propagate, GlobalSignalRuntime);

  let _source = subscription_with_runtime!(|| (), GlobalSignalRuntime);

  let _effect = effect_with_runtime!(|| (), drop, GlobalSignalRuntime);
}
```

Runtime have some leeway regarding in which order they invoke the callbacks. A different runtime may also choose to merge propagation from distinct updates.
