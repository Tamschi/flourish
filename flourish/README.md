# *flourish*

Convenient and full-featured signals for Rust.

The API design emphasises efficient resource management and performance-aware code without compromising on ease of use at near-zero boilerplate.

🚧 This is a(n optimisable) proof of concept! The API is full-featured, but the code is not (much at all) optimised. However, high degrees of optimisation should be possible without breaking changes. 🚧

*flourish* is a signals library inspired by [🚦 JavaScript Signals standard proposal🚦](https://github.com/tc39/proposal-signals?tab=readme-ov-file#-javascript-signals-standard-proposal) (but Rust-y).

When combined with for example [`Option`](https://doc.rust-lang.org/stable/core/option/enum.Option.html) and [`Future`](https://doc.rust-lang.org/stable/core/future/trait.Future.html), *flourish* can model asynchronous-and-cancellable resource loads efficiently.

This makes it a suitable replacement for most standard use cases of RxJS-style observables, though *with the included runtime* it **may debounce propagation and as such isn't suited for sequences**. (You should probably prefer channels for those. *flourish* does work well with reference-counted resources, however, and can flush them from stale unsubscribed signals.)

**Distinct major versions of this library are logically cross-compatible**, as long as they use the same version of `isoprenoid`.

## Known Issues

⚠️ The update task queue is currently not fair whatsoever, so one thread looping inside signal processing will block all others.  
(You *can* substitute your own `SignalsRuntimeRef` implementation if you'd like to experiment. All relevant types in this crate are generic over the runtime, so that which you're working with is easy to identify or preset via type alias.)

⚠️ The panic handling in the included runtime isn't good yet.  
Fixing this doesn't incur API changes, and I don't need it right now, so I haven't implemented panic routing that would preserve the runtime when callbacks fail.

## Prelude

*flourish*'s prelude re-exports its unmanaged accessor traits and the `SignalsRuntimeRef` trait. *You need neither to work with managed signals*, but are likely to make use of the traits for custom low-level combinators.

If you can't call `.get()` or `.change(…)` on pinned unmanaged signals, this import is what you're looking for:

```rust
use flourish::prelude::*;
```

## Quick-Start

For libraries (which should be generic over the signals runtime `SR`):

```sh
cargo add flourish
```

For applications ("batteries included"):

```sh
cargo add flourish --features global_signals_runtime
```

You can put signals on the heap:

```rust
use flourish::{Propagation, GlobalSignalsRuntime, SignalArcDynCell, SignalArcDyn};

// Choose a runtime:
type Effect<'a> = flourish::Effect<'a, GlobalSignalsRuntime>;
type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;
type Subscription<T, S> = flourish::Subscription<T, S, GlobalSignalsRuntime>;

// `Signal` is a ref-only type like `Path`, so its constructors return a `SignalArc`.
let _ = Signal::cell(());
let _ = Signal::cell_cyclic(|_weak| ());
let _ = Signal::cell_reactive((), |_value, _status| Propagation::Halt);
let _ = Signal::cell_reactive_mut((), |_value, _status| Propagation::Propagate);
let _ = Signal::cell_cyclic_reactive(|_weak| ((), move |_value, _status| Propagation::Halt));
let _ = Signal::cell_cyclic_reactive_mut(|_weak| ((), move |_value, _status| Propagation::Propagate));

// Not evaluated unless subscribed.
let _ = Signal::computed(|| ());
let _ = Signal::debounced(|| ());
let _ = Signal::computed_uncached(|| ()); // `Fn` closure. The others take `FnMut`s.
let _ = Signal::computed_uncached_mut(|| ());
let _ = Signal::folded((), |_value| Propagation::Propagate);
let _ = Signal::reduced(|| (), |_value, _next| Propagation::Propagate);

// `Subscription` is the subscribed form of `SignalArc`.
let _ = Subscription::computed(|| ());
let _ = Subscription::folded((), |_value| Propagation::Propagate);
let _ = Subscription::reduced(|| (), |_value, _next| Propagation::Propagate);

// Runs `drop` *before* computing the new value.
// The effect closures' types are always erased.
let _ = Effect::new(|| (), drop);

// "Splitting":
let (_signal, _cell) = Signal::cell(()).into_read_only_and_self();

// Erase the unmanaged/closure type:
let _: SignalArcDynCell<(), GlobalSignalsRuntime> = Signal::cell(()).into_dyn_cell();
let _: SignalArcDyn<(), GlobalSignalsRuntime> = Signal::computed(|| ()).into_dyn();
let (_signal_dyn, _cell_dyn) = Signal::cell(()).into_dyn_read_only_and_self();
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

Additionally, inside `flourish::raw`, you can find constructor functions for unpinned unmanaged signals that enable composition with data-inlining.

## Linking signals

`flourish` detects and updates dependencies automatically:

```rust
use flourish::{shadow_clone, GlobalSignalsRuntime};

// Choose a runtime:
type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;

let a = Signal::cell("a");
let b = Signal::cell("b");
let c = Signal::cell("c");
let d = Signal::cell("d");
let e = Signal::cell("e");
let f = Signal::cell("f");
let g = Signal::cell("g");
let index = Signal::cell(0);

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

// For demo purposes, the original `SignalArc` is preserved here.
// To consume it, write `.into_subscription()`, which is more efficient.
let subscription = signal.to_subscription(); // ""

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
index.change(3); // nothing, even though `signal` still exists

drop(signal);
```

`Signal`s are fully lazy, so they generally only run their closures while subscribed or to refresh their value if dirty.

The default `GlobalSignalsRuntime` notifies signals iteratively from earlier to later when possible. Only one such notification cascade is processed at a time with this runtime.

("uncached" signals run their closure whenever their value is retrieved instead, not on update.)

## Unsizing

As mentioned in passing earlier, closure types captured in signals in this library can be erased from smart pointers and references. For example:

```rust

use flourish::{shadow_clone, GlobalSignalsRuntime, Propagation};

// Choose a runtime:
type Signal<T, S> = flourish::Signal<T, S, GlobalSignalsRuntime>;

let mut cell;
cell = Signal::cell(()).into_dyn_cell();
cell = Signal::cell_reactive((), |_, _| Propagation::Halt).into_dyn_cell();
cell = Signal::cell_reactive((), |_, _| Propagation::Halt).into(); // via `Into`

let mut signal;
signal = Signal::cell(()).into_dyn();
signal = Signal::cell_reactive((), |_, _| Propagation::Halt).into_dyn();
signal = Signal::cell_reactive((), |_, _| Propagation::Halt).into(); // via `Into`
signal = Signal::computed(|| ()).into_dyn();
signal = Signal::computed(|| ()).into(); // via `Into`
```

There are additional conversion methods available. See the `conversions` module for details.

## Using an instantiated runtime

You can use existing `isoprenoid` runtime instances with the included types and macros (but ideally, still alias these items for your own use):

```rust
use flourish::{signals_helper, GlobalSignalsRuntime, Propagation, Signal, Subscription};

let _ = Signal::cell_with_runtime((), GlobalSignalsRuntime);

let _ = Signal::computed_with_runtime(|| (), GlobalSignalsRuntime);
let _ = Signal::computed_uncached_with_runtime(|| (), GlobalSignalsRuntime);
let _ = Signal::computed_uncached_mut_with_runtime(|| (), GlobalSignalsRuntime);
let _ = Signal::folded_with_runtime((), |_value| Propagation::Propagate, GlobalSignalsRuntime);
let _ = Signal::reduced_with_runtime(|| (), |_value, _next| Propagation::Propagate, GlobalSignalsRuntime);

let _ = Subscription::computed_with_runtime(|| (), GlobalSignalsRuntime);

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

Runtimes have some leeway regarding when and in which order they invoke the callbacks. They can also decide whether to perform all updates' effects separately or merge refresh cascades.

## Compiler (and Standard Library) Wishlist

Several improvements to this library are postponed pending certain compiler features (getting stabilised).

This mainly affects certain optimisations not being in place yet, but does have some small effects on the API where I had to use workarounds.

|Feature|What it would enable|
|-|-|
|[`coerce_unsized`](https://github.com/rust-lang/rust/issues/18598)|More type-erasure coercions for various `Signal` handle types (probably). For now, please use the respective conversion methods or `From`/`Into` conversions instead.|
|[`trait_upcasting`](https://github.com/rust-lang/rust/issues/65991)|Conversions from `…DynCell` to `…Dyn`.|
|Fix for [Unexpected higher-ranked lifetime error in GAT usage](https://github.com/rust-lang/rust/issues/100013)|(Cleanly) avoid boxing the inner closure in many "`_eager`" methods.|
|Object-safety for `trait Guard: Deref + Borrow<Self::Target> {}` as `dyn Guard<Target = …>`|I think this is caused by use of the associated type as type parameter in any bound (of `Self` or an associated type). It works fine with `Guard<T>`, but that's not ideal since `Guard` is implicitly unique per implementing type (and having the extra generic type parameter complicates some other code).|
|[`type_alias_impl_trait`](https://github.com/rust-lang/rust/issues/63063)|Eliminate boxing and dynamic dispatch of `Future`s in some static-dispatch methods of signal cell implementations.|
|[`impl_trait_in_assoc_type`](https://github.com/rust-lang/rust/issues/63063)|Eliminate several surfaced internal types, resulting in better docs.|
|[Precise capturing in RPITIT](https://github.com/rust-lang/rust/pull/126746)|This would clean up the API quite a lot, by removing some GATs.|
|Deref coercions in constant functions|Make several conversions available as `const` methods.|
|[`arbitrary_self_types`](https://github.com/rust-lang/rust/issues/44874)|Inline-pinning of values (with a clean API).|
|`Pin<Ptr: ?Sized>`|Type-erasure for the aforementioned clean inline-pinning signals.|
|["`super let`"](https://blog.m-ou.se/super-let/) (or equivalent)|Easier-to-use macros for unmanaged/inline signals.|
|"`FnPin`" and "`FnPinMut`" closures with simple return type, also implemented by current `FnMut` closures and functions | This could nicely allow safe `\|\| { let x = pin!(…); loop { yield …; } }` closures for the "fn_pin" parameters, where currently only `FnMut` is accepted and any inline pinning requires `unsafe`.|

## Open Questions

- Would a `WeakSubscription` be useful? It would keep a `Signal` subscribed without preventing its destruction.

  On one hand that may be useful to keep certain caches fresh. On the other hand, it would make it *a lot* easier to cause hard-to-debug side effects.

- `Signal` doesn't have `.as_unmanaged()` or `.as_unmanaged_cell()` methods (`(&self) -> Pin<&impl 'a + Unmanaged…>`) because that would give access to the unmanaged `.subscribe()` and `.unsubscribe()` which, while safe, are easy to misuse. Shimming this is easy, but comes with a little overhead.

  How important is it to have a common trait for value access and cell updates here?
  (It's quite complicated due to dyn-compatibility rules, as in `Signal` access to those methods is
  controlled by `S: Sized` instead of `Self: Sized`.)
