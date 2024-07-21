# flourish (isoprenoid)

TODO

## Documentation

Please build the documentation with either of:

```sh
cargo +nightly -Z package-features doc --features _docs
cargo +nightly -Z package-features doc --features _docs --open
```

## Testing

Run tests with all of:

```sh
cargo +stable test
cargo +nightly -Z package-features test --features _test
cargo +nightly -Z package-features miri test --features _test
```

Most tests require the included global signals runtime, but should still compile without it.
