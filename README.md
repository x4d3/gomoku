# Gomoku (Infinite 5-in-a-row) — Rust → WebAssembly

Unbounded-grid Gomoku with a heuristic AI, compiled to WebAssembly.
Controls: Click to place, Arrow keys to pan, `R` to restart, `+`/`-` to zoom.

You can play agains AI here: https://x4d3.github.io/gomoku/

## Build
1) Install toolchain:
   - Rust: https://rustup.rs
   - wasm target: `rustup target add wasm32-unknown-unknown`
   - wasm-bindgen-cli: `cargo install wasm-bindgen-cli`
   - `cargo install --locked trunk`

2) Serve
```
trunk serve --open
```

2) Build
```
trunk build
```