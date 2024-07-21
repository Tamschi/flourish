# flourish (isoprenoid)

TODO

## Testing

Run tests with both

```sh
cargo +stable test
cargo +nightly -Z package-features test --features _test
```

Most tests require the included global signals runtime, but should still compile without it.
