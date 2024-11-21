# `isoprenoid`

`isoprenoid` is the signals runtime framework backing *flourish*.

Distinct major versions of *flourish* are compatible as long as they use the same version of `isoprenoid`.

## Features

### `"global_signals_runtime"`

Implements `SignalsRuntimeRef` for `GlobalSignalsRuntime`.

### `"forbid_global_signals_runtime"`

Asserts that `"global_signals_runtime"` is not enabled.

## Quick-start

- To create your own signals runtime, implement [`runtime::SignalsRuntimeRef`].
- To easily create a compatible alternative to Flourish, wrap [`raw::RawSignal`].
  - For tighter integration with Flourish, implement its `UnmanagedSignal` and optionally `UnmanagedSignalCell` traits.
- To write application code, use only Flourish instead.

## Threading Notes

Please note that *none* of the function in this library are guaranteed to produce *any* memory barriers!
