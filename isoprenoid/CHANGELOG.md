# `isoprenoid` Changelog

## 0.1.3

2025-12-16

Revisions:
  - Fixed a logic bug that could potentially have resulted in signals being refreshed twice iff such a refresh was scheduled both due to flushing and non-flushing propagation.  
    (The behaviour continues to not be guaranteed either way, but now is potentially more efficient in some edge cases.)

## 0.1.2

2025-06-02

- Features:
  - Added `SignalsRuntimeRef::hint_batched_updates` method with default implementation.
  - Wasm-compatibility.

- Revisions:
  - Documentation typo fix.

## 0.1.1

2024-11-22

- Revisions:
  - Fixed docs.rs build.

## 0.1.0

2024-11-22

Initial release.
