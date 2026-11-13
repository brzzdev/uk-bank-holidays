# uk-bank-holidays

A desktop app for UK bank holidays, built in Rust on [GPUI](https://gpui.rs/) and
[gpui-kit](https://gpui-kit.com/).

Early days: the workspace is scaffolded, and the app itself is still to come. It holds three crates:

- `bank-holidays` (`crates/lib`): the library
- `bank-holidays-cli` (`crates/cli`): the command-line tool
- `bank-holidays-app` (`crates/app`): the macOS app, on gpui-kit

`cargo run -p bank-holidays-app` opens the app. It needs macOS 15 or newer.

## Development

Needs Rust 1.100 or newer, [`just`](https://just.systems), and, for `just install-app`, `jq`.

```sh
just test          # run the test suite
just lint          # clippy, warnings as errors
just format        # rustfmt
just install-app   # build Bank Holidays.app and install it in /Applications
just tools         # install a pre-commit hook running format-check and lint
```

CI runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` across the
workspace on macOS 15, on every pull request.

## License

MIT. See [LICENSE](LICENSE).
