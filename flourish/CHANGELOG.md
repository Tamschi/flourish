# *flourish* Changelog

## 0.2.0+0.1-compatible

2025-12-16

- Breaking Changes:
  - Streamlines signal cell API:
    - "`change`" is now "`replace_if_distinct`" to clarify what it does.
      - All variants are renamed accordingly.
    - `replace()` is now `set()`, as it (generally, but without guarantee) overwrite the old value in place.

- Features:
  - Added "`set`" and "`set_if_distinct`" variant methods that (generally, but without guarantee) overwrite the old value in place.

These changes are reflected on both `UnmanagedSignalCell` and the cell API on `Signal`.

## 0.1.3

2025-06-02

- Features:
  - Wasm-compatibility.

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
