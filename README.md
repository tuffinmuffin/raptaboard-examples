# raptaboard-examples

Example widget plugins for [raptaboard](https://github.com/tuffinmuffin/raptaboard), the FRC
driver dashboard: each one shows how to write a plugin, and they can be installed from
raptaboard's content packs ("Example widget plugins").

- [number](number/): a number topic as text, like raptaboard's built-in Text Display.
- [alerts](alerts/): a WPILib Alerts group as a list, like the built-in Alerts widget: a sound
  for a new error if turned on, and its tab's mark while the tab is closed.

A plugin is a WebAssembly component plus its `plugin.toml`. The interface is raptaboard's
`wit/dashboard.wit`; the plugins use its Rust SDK (`raptaboard-plugin`) and test with its harness
(`raptaboard-plugin-test`), both from raptaboard at the commit pinned in [Cargo.toml](Cargo.toml).

## Building and testing

```sh
rustup target add wasm32-wasip2
cargo build --release --target wasm32-wasip2   # target/wasm32-wasip2/release/<name>.wasm
cargo test                                     # each plugin's tests, in raptaboard's runtime
```

Try one in raptaboard's sandbox window, from a raptaboard checkout:

```sh
mkdir -p /tmp/number && cp ../raptaboard-examples/number/plugin.toml /tmp/number/ \
  && cp ../raptaboard-examples/target/wasm32-wasip2/release/number.wasm /tmp/number/widget.wasm
cargo run -- plugin dev /tmp/number
```

## Releases

Push a tag on `main` to publish: `vX.Y.Z` makes a release, `vX.Y.Z-pre.N` a pre-release. The
Release workflow tests and builds the plugins and attaches each as `<name>.wasm` with its manifest
as `<name>.toml` (and `SHA256SUMS`). raptaboard-content's packs point at these files; raise a
plugin's `version` in its `plugin.toml` when it changes.

## License

MIT; see [LICENSE](LICENSE).
