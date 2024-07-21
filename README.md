# flourish (isoprenoid)

TODO

## Documentation

Please build the documentation with either of:

```sh
cargo +stable doc --features _doc
cargo +stable doc --features _doc --open
```

## Testing

Run tests with all of:

```sh
cargo +stable test
cargo +stable test --features _test
cargo +nightly miri test --features _test
```

Most tests require the included global signals runtime, but should still compile without it.

Please also check for unused dependencies using [cargo-udeps](https://lib.rs/crates/cargo-udeps) with both:

```sh
cargo +nightly udeps
cargo +nightly udeps --features _test
```

Dependencies that are used only with certain features should be optional.
