# Gomoku (Infinite 5-in-a-row) — Rust → WebAssembly

Unbounded-grid Gomoku with a heuristic AI, compiled to WebAssembly.
Controls: Click to place, Arrow keys to pan, `R` to restart, `+`/`-` to zoom.

You can play agains AI here: https://x4d3.github.io/gomoku/

## Build
1) Install toolchain:
   - Rust: https://rustup.rs
   - wasm target: `rustup target add wasm32-unknown-unknown`
   - wasm-bindgen-cli: `cargo install wasm-bindgen-cli`

2) Build:
```
cargo build --release --target wasm32-unknown-unknown
```

3) Generate JS bindings (outputs to ./web):
```
wasm-bindgen --target web \
  --out-dir docs \
  --no-typescript \
  target/wasm32-unknown-unknown/release/gomoku.wasm
```

4) Serve `docs/`:
```
cd docs && python -m http.server 8000
# open http://localhost:8000
```
