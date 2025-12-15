# `isoprenoid-unsend`

`isoprenoid-unsend` is the signals runtime framework backing *flourish-unsend*.  
It's a thread-local alternative/variant of `isoprenoid` that can be used with `!Send` values.

Distinct major versions of *flourish-unsend* are compatible as long as they use the same version of `isoprenoid-unsend`.

## Features

### `"local_signals_runtime"`

Implements `SignalsRuntimeRef` for `LocalSignalsRuntime`.

### `"forbid_local_signals_runtime"`

Asserts that `"local_signals_runtime"` is not enabled.

## Quick-start

- To create your own signals runtime, implement [`runtime::SignalsRuntimeRef`].
- To easily create a compatible alternative to *flourish-unsend*, wrap [`raw::RawSignal`].
  - For tighter integration with *flourish-unsend*, implement its `UnmanagedSignal` and optionally `UnmanagedSignalCell` traits.
- To write application code, use only *flourish-unsend* instead.
