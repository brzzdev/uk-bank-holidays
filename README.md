# uk-bank-holidays

A desktop app for UK bank holidays, built in Rust on [GPUI](https://gpui.rs/) and
[gpui-kit](https://gpui-kit.com/).

Early days: the crate is scaffolded, and the app itself is still to come.

## Development

Needs Rust 1.99 or newer and [`just`](https://just.systems).

```sh
just test          # run the test suite
just lint          # clippy, warnings as errors
just format        # rustfmt
just tools         # install a pre-commit hook running format-check and lint
```

CI runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` on every
pull request.

## License

MIT. See [LICENSE](LICENSE).
