# `flourish`

🚧 This is a(n optimisable) proof of concept! 🚧

Flourish is a signals library inspired by [🚦 JavaScript Signals standard proposal🚦](https://github.com/tc39/proposal-signals?tab=readme-ov-file#-javascript-signals-standard-proposal) (but Rust-y).

## Quick-Start

You can put signals on the heap:

```rust
use flourish::{Signal, Subject, Subscription, Update};

let _ = Subject::new(());

// The closure type is erased!
// Not evaluated unless subscribed.
let _ = Signal::computed(|| ());
let _ = Signal::computed_uncached(|| ());
let _ = Signal::computed_uncached_mut(|| ());
let _ = Signal::folded((), |_value| Update::Propagate);
let _ = Signal::merged(|| (), |_value, _next| Update::Propagate);

// The closure type is erased!
let _ = Subscription::computed(|| ());
```

You can also put signals on the stack:

```rust
use flourish::{signals_helper, Update};

signals_helper! {
  let _subject = subject!(());

  // The closure type is erased!
  // Not evaluated unless subscribed.
  let _source = computed!(|| ());
  let _source = computed_uncached!(|| ());
  let _source = computed_uncached_mut!(|| ());
  let _source = folded!((), |_value| Update::Propagate);
  let _source = merged!(|| (), |_value, _next| Update::Propagate);

  // The closure type is erased!
  let _source = subscription!(|| ());
}
```

## Linking signals

`flourish` detects and updates dependencies automatically:

```rust
use flourish::{shadow_clone, Signal, Subject, Subscription, Update};

let a = Subject::new("a");
let b = Subject::new("b");
let c = Subject::new("c");
let d = Subject::new("d");
let e = Subject::new("e");
let f = Subject::new("f");
let g = Subject::new("g");
let index = Subject::new(0);

let subscription = Subscription::computed({
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
}); // ""

a.set("a"); b.set("b"); // nothing
index.set(1); // "a"
a.set("aa"); // "aa"
b.set("bb"); // nothing
index.set(2); // "bb"
a.set("a"); // nothing
b.set("b"); // "b"

drop(subscription);
index.set(3); // nothing
```

`Signal`s are fully lazy, so they only update while subscribed or to refresh their value if dirty.  
("uncached" signals run their closure whenever their value is retrieved, but not on update.)

## Using a different runtime

You can use a different [`pollinate`] runtime with the included types and macros (but ideally, alias these items for your own use):

```rust
use flourish::{signals_helper, GlobalSignalRuntime, SignalSR, Subject, SubscriptionSR, Update};

let _ = Subject::with_runtime((), GlobalSignalRuntime);

let _ = SignalSR::computed_with_runtime(|| (), GlobalSignalRuntime);
let _ = SignalSR::computed_uncached_with_runtime(|| (), GlobalSignalRuntime);
let _ = SignalSR::computed_uncached_mut_with_runtime(|| (), GlobalSignalRuntime);
let _ = SignalSR::folded_with_runtime((), |_value| Update::Propagate, GlobalSignalRuntime);
let _ = SignalSR::merged_with_runtime(|| (), |_value, _next| Update::Propagate, GlobalSignalRuntime);

let _ = SubscriptionSR::computed_with_runtime(|| (), GlobalSignalRuntime);

signals_helper! {
  let _subject = subject_with_runtime!((), GlobalSignalRuntime);

  let _source = computed_with_runtime!(|| (), GlobalSignalRuntime);
  let _source = computed_uncached_with_runtime!(|| (), GlobalSignalRuntime);
  let _source = computed_uncached_mut_with_runtime!(|| (), GlobalSignalRuntime);
  let _source = folded_with_runtime!((), |_value| Update::Propagate, GlobalSignalRuntime);
  let _source = merged_with_runtime!(|| (), |_value, _next| Update::Propagate, GlobalSignalRuntime);

  let _source = subscription_with_runtime!(|| (), GlobalSignalRuntime);
}
```

The runtime has some leeway regarding in which order it invokes the callbacks.

## TODO

```rust
use flourish::{shadow_clone, Signal, Subject, Subscription};

let a = Subject::new(1); // : Subject<i32>
let b = Subject::new(2); // : Subject<i32>
let sum = Signal::computed({
  shadow_clone!(a, b);
  move || a.get() + b.get()
}); // : Signal<i32>
let subscription = Subscription::computed({
  shadow_clone!(a, b, sum);
  move || println!("{} + {} = {}", a.get(), b.get(), sum.get())
}); // : Subscription<()>, "1 + 2 = 3"

a.set(2); // "2 + 2 = 4"
b.set(3); // "2 + 3 = 5"
```

And on the stack:

```rust
use flourish::{signals_helper, Source};

signals_helper! {
  let a = subject!(1); // : Pin<&RawSubject<i32>>
  let b = subject!(2); // : Pin<&RawSubject<i32>>
  let sum = computed!(|| a.get() + b.get()); // : Pin<&impl Source<_, Value = i32>>
  let _subscription = subscription!(|| println!("{} + {} = {}", a.get(), b.get(), sum.get()));
  // : Pin<&impl Source<_, Value = ()>>, "1 + 2 = 3"
}

a.set(2); // "2 + 2 = 4"
b.set(3); // "2 + 3 = 5"
```

You can specify a runtime to adjust the scheduling:

```rust
use flourish::{shadow_clone, GlobalSignalRuntime, SignalSR, Subject, SubscriptionSR};

let a = Subject::with_runtime(1, GlobalSignalRuntime); // : SubjectSR<i32, _>
let b = Subject::with_runtime(2, GlobalSignalRuntime); // : Subject<i32, _>
let sum = SignalSR::computed_with_runtime({
  shadow_clone!(a, b);
  move || a.get() + b.get()
}, GlobalSignalRuntime); // : SignalSR<i32, _>
let subscription = SubscriptionSR::computed_with_runtime({
  shadow_clone!(a, b, sum);
  move || println!("{} + {} = {}", a.get(), b.get(), sum.get())
}, GlobalSignalRuntime); // : SubscriptionSR<(), _>, "1 + 2 = 3"

a.set(2); // "2 + 2 = 4"
b.set(3); // "2 + 3 = 5"
```

(In practice, consider `type` aliases similar to `Signal` and `Subscription`.)

//TODO: Revise this; closures should probably require explicit use of `cached`, `computed` and `volatile`. Or more likely, a reduced set among them.

`Fn() -> T` closures and `(closure, runtime)` tuples can be used directly in place of other raw (inlined) sources. To make use of `FnMut() -> T` closures, use `computed` (caching) and `volatile` (locking):

```rust
//TODO
```
