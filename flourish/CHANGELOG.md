# *flourish* Changelog

## next

TODO: Date

- Breaking:
  - Removed `Into` conversion from unmanaged signals to `SignalArc`.  
    (Use `SignalArc::new` instead.)
  - Added conversions from `T` for owned signals and handles, either without closures.  
    (This makes `.into()` ambiguous in some additional cases. Use specific `.into_…()` methods where necessary.)

- Features:
  - Added `Signal::shared` and `Signal::shared_with_runtime`, which create lightweight untracked wrappers around `Sync` values.

- Revisions:
  - README fix: `flourish::raw` has been `flourish::unmanaged` for a while.
  - README formatting fixes.
  - Added test hello_flourish.

## 0.1.1

2024-11-22

- Revisions:
  - Fixed docs.rs build.

## 0.1.0

2024-11-22

Initial release.
