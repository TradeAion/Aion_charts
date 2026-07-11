# Aion Charts

A production trading-chart engine in **Rust + WebGPU + WASM**, pixel-faithful to
[lightweight-charts](https://github.com/tradingview/lightweight-charts) with a long-term
trajectory toward a full TradingView-class charting platform.

## Documents

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — system design, crate layout, roadmap.
- [docs/RENDERING_SPEC.md](docs/RENDERING_SPEC.md) — exact pixel math ported from
  lightweight-charts (the fidelity contract; every renderer must match it).

## Workspace

| Crate | Purpose |
|---|---|
| `crates/aion_core` | Platform-free chart model: scales, ranges, invalidation, formatters |
| `crates/aion_render` | Draw-list IR + rendering math (bar widths, primitives) |
| `crates/aion_render_wgpu` | WebGPU backend (pipelines, glyph atlas) — WIP |
| `crates/aion_wasm` | wasm-bindgen host shell (DOM, events, RAF) — WIP |
| `packages/charts` | Public TypeScript API (`@aion/charts`), snake_case — WIP |

Naming convention: **snake_case everywhere**, including the public TS API.

## Development

```sh
cargo test          # core math + rendering math unit tests
cargo clippy --all-targets
```

The vendored lightweight-charts source in `tmp/` is a study reference only and is git-ignored.
