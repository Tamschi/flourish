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
