# *flourish* Changelog

## next major

TODO: Date

- Breaking:
  - Removed `Into` conversion from unmanaged signals to `SignalArc`.  
    (Use `SignalArc::new` instead.)
  - Revised cell API. There are new `set…` methods that **may** be implemented to drop the old value in place, and `change…` is now `replace_distinct…` and now requires `Eq` rather than just `PartialEq`.

## next

TODO: Date

- Revisions:
  - Conversion table formatting fix.
  - Fixed changelog heading "0.1.2".

## 0.1.2

2025-04-03

- Features:
  - Added functions `Signal::shared`, `Signal::shared_with_runtime` and `unmanaged::shared`, which create lightweight untracked wrappers around `Sync` values.
    - `unmanaged::shared` can also be used through the `unmanaged::signals_helper!` macro.
  - Upcasting conversions are now available (generics omitted):
    - `SignalDynCell::as_read_only(&self) -> &SignalDyn`
    - `SignalDynCell::to_read_only(&self) -> &SignalArcDyn`
    - `SignalArcDynCell::into_read_only(self) -> SignalArcDyn`
    - `SignalArcDynCell::into_read_only_and_self(self) -> (SignalArcDyn, Self)`
    - `SignalWeakDynCell::into_read_only(self) -> SignalWeakDyn`
    - `SignalWeakDynCell::into_read_only_and_self(self) -> (SignalWeakDyn, Self)`
    - `SubscriptionDynCell::into_read_only(self) -> SubscriptionDyn`
    - Upcasting `From` and `TryFrom` implementations (for side-effect-free conversions).
  - Added unsizing and upcasing `From` implementations between unmanaged signal references.

- Revisions:
  - README fix: `flourish::raw` has been `flourish::unmanaged` for a while.
  - README formatting fixes.
  - Added tests "hello_flourish" and "upcasting".
  - The MSRV is now 1.86 (which is required for trait upcasting).

Note that upcasting coercions are largely not yet available (except from `SignalDynCell` to `SignalDyn`), as this would likely require [`coerce_unsized`](https://github.com/rust-lang/rust/issues/18598) to be stabilised.

## 0.1.1

2024-11-22

- Revisions:
  - Fixed docs.rs build.

## 0.1.0

2024-11-22

Initial release.
