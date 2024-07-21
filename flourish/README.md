# `flourish`

Convenient and composable signals for Rust.

The API design emphasises efficient resource management and performance-aware code without compromising on ease of use at near-zero boilerplate.

🚧 This is a(n optimisable) proof of concept! 🚧  
🚧 The API is full-featured, but the code is not (much at all) optimised. 🚧

Flourish is a signals library inspired by [🚦 JavaScript Signals standard proposal🚦](https://github.com/tc39/proposal-signals?tab=readme-ov-file#-javascript-signals-standard-proposal) (but Rust-y).

When combined with for example [`Option`](https://doc.rust-lang.org/stable/core/option/enum.Option.html) and [`Future`](https://doc.rust-lang.org/stable/core/future/trait.Future.html), `flourish` can model asynchronous-and-cancellable resource loads. See the crate `flourish-extra` for example combinators and `flourish-extensions` to use them conveniently through constructor extensions.

This makes it a suitable replacement for most standard use cases of RxJS-style observables.

## Known Issues

⚠️ The update task queue is currently not fair whatsoever, so one thread looping inside signal processing will block all others.

## Prelude

Flourish's prelude re-exports its accessor traits anonymously.

If you can't call `.get()` or `.change(…)`, this import is what you're looking for:

```rust
use flourish::prelude::*;
```

## Quick-Start

For libraries (which should be generic over the signals runtime `SR`):

```sh
cargo add flourish
```

For applications:

```sh
cargo add flourish --features global_signals_runtime
```

You can put signals on the heap:

```rust
use flourish::{SignalCell, Propagation, Signal, Subscription, Effect};

let _ = SignalCell::new(());
let _ = SignalCell::new_cyclic(|_weak| ());
let _ = SignalCell::new_reactive((), |_value, _status| Propagation::Halt);
let _ = SignalCell::new_reactive_mut((), |_value, _status| Propagation::Propagate);
let _ = SignalCell::new_cyclic_reactive(|_weak| ((), move |_value, _status| Propagation::Halt));
let _ = SignalCell::new_cyclic_reactive_mut(|_weak| ((), move |_value, _status| Propagation::Propagate));

// The closure type is erased!
// Not evaluated unless subscribed.
let _ = Signal::computed(|| ());
let _ = Signal::debounced(|| ());
let _ = Signal::computed_uncached(|| ()); // `Fn` closure. The others take `FnMut`s.
let _ = Signal::computed_uncached_mut(|| ());
let _ = Signal::folded((), |_value| Propagation::Propagate);
let _ = Signal::reduced(|| (), |_value, _next| Propagation::Propagate);

// The closure type is erased!
let _ = Subscription::computed(|| ());
let _ = Subscription::folded((), |_value| Propagation::Propagate);
let _ = Subscription::reduced(|| (), |_value, _next| Propagation::Propagate);

// The closure and value type are erased!
// Runs `drop` *before* computing the new value.
let _ = Effect::new(|| (), drop);

// "Splitting":
let (_signal, _cell) = SignalCell::new(()).into_signal_and_self();
let (_signal, _type_erased_cell) = SignalCell::new(()).into_signal_and_erased();
```

You can also put signals on the stack:

```rust
use flourish::{signals_helper, prelude::*, Propagation};

signals_helper! {
  let inert_cell = inert_cell!(());
  let reactive_cell = reactive_cell!((), |_value, _status| Propagation::Halt);

  // The closure type is erased!
  // Not evaluated unless subscribed.
  let _source = computed!(|| ());
  let _source = debounced!(|| ());
  let _source = computed_uncached!(|| ());
  let _source = computed_uncached_mut!(|| ());
  let _source = folded!((), |_value| Propagation::Propagate);
  let _source = reduced!(|| (), |_value, _next| Propagation::Propagate);

  // The closure type is erased!
  let _source = subscription!(|| ());

  // Runs `drop` *before* computing the new value.
  let _effect = effect!(|| (), drop);
}

// "Splitting":
let (_source, _source_cell) = inert_cell.as_source_and_cell();
let (_source, _source_cell) = reactive_cell.as_source_and_cell();
```

Additionally, inside `flourish::raw`, you can find constructor functions for unpinned raw signals that enable composition with data-inlining.

## Linking signals

`flourish` detects and updates dependencies automatically:

```rust
use flourish::{prelude::*, shadow_clone, SignalCell, Signal, Subscription};

let a = SignalCell::new("a");
let b = SignalCell::new("b");
let c = SignalCell::new("c");
let d = SignalCell::new("d");
let e = SignalCell::new("e");
let f = SignalCell::new("f");
let g = SignalCell::new("g");
let index = SignalCell::new(0);

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

The default `GlobalSignalsRuntime` notifies signals iteratively from earlier to later when possible. Only one such notification cascade is processed at a time with this runtime.

("uncached" signals run their closure whenever their value is retrieved instead, not on update.)

## Using a different runtime

You can use a different [`isoprenoid`] runtime with the included types and macros (but ideally, alias these items for your own use):

```rust
use flourish::{signals_helper, GlobalSignalsRuntime, SignalSR, SignalCell, SubscriptionSR, Propagation};

let _ = SignalCell::with_runtime((), GlobalSignalsRuntime);

let _ = SignalSR::computed_with_runtime(|| (), GlobalSignalsRuntime);
let _ = SignalSR::computed_uncached_with_runtime(|| (), GlobalSignalsRuntime);
let _ = SignalSR::computed_uncached_mut_with_runtime(|| (), GlobalSignalsRuntime);
let _ = SignalSR::folded_with_runtime((), |_value| Propagation::Propagate, GlobalSignalsRuntime);
let _ = SignalSR::reduced_with_runtime(|| (), |_value, _next| Propagation::Propagate, GlobalSignalsRuntime);

let _ = SubscriptionSR::computed_with_runtime(|| (), GlobalSignalsRuntime);

signals_helper! {
  let _inert_cell = inert_cell_with_runtime!((), GlobalSignalsRuntime);

  let _source = computed_with_runtime!(|| (), GlobalSignalsRuntime);
  let _source = computed_uncached_with_runtime!(|| (), GlobalSignalsRuntime);
  let _source = computed_uncached_mut_with_runtime!(|| (), GlobalSignalsRuntime);
  let _source = folded_with_runtime!((), |_value| Propagation::Propagate, GlobalSignalsRuntime);
  let _source = reduced_with_runtime!(|| (), |_value, _next| Propagation::Propagate, GlobalSignalsRuntime);

  let _source = subscription_with_runtime!(|| (), GlobalSignalsRuntime);

  let _effect = effect_with_runtime!(|| (), drop, GlobalSignalsRuntime);
}
```

Runtimes have some leeway regarding in which order they invoke the callbacks. A different runtime may also choose to combine propagation from distinct updates, reducing the amount of callback runs.
