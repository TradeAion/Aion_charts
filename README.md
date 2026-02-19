# RayCore

**High-performance, GPU-accelerated charting engine built in Rust + WebGPU.**

Created by [RayCharts](https://raycharts.com/) — all rights reserved.

---

## Overview

RayCore is a from-scratch charting engine that renders financial candlestick charts entirely on the GPU via WebGPU. It is compiled to WebAssembly for browser deployment, delivering native-level rendering performance in the browser with zero JavaScript rendering overhead.

This is **not** a wrapper around Canvas 2D or SVG. Every candle body, wick, volume bar, and grid line is drawn through GPU-instanced draw calls using custom WGSL shaders.

---

## Architecture

```
raycharts_core/
├── src/                        # Core Rust library (raycore)
│   ├── lib.rs                  # Public API & re-exports
│   └── core/
│       ├── engine.rs           # ChartEngine — orchestrates data + rendering
│       ├── data.rs             # OHLCV data model
│       ├── viewport.rs         # Viewport state (pan, zoom, visible range)
│       └── renderer/
│           ├── traits.rs       # ChartRenderer / RendererBackend trait
│           ├── wgpu_context.rs # GPU device, surface, adapter init
│           ├── wgpu_backend.rs # WgpuRenderer — main GPU render loop
│           ├── pipeline_manager.rs # Shader pipeline cache
│           ├── candle_renderer.rs  # Instanced candlestick rendering
│           ├── volume_renderer.rs  # Volume bar rendering
│           ├── price_axis.rs   # Price axis (Canvas 2D)
│           ├── time_axis.rs    # Time axis (Canvas 2D)
│           ├── overlay.rs      # Crosshair & tooltip overlay
│           ├── grid.rs         # Grid line generation
│           ├── tick_marks.rs   # Axis tick calculation
│           ├── theme.rs        # Color & style constants
│           ├── geometry_generator.rs
│           ├── series.rs
│           ├── canvas2d.rs
│           └── draw_list.rs
├── wasm/                       # WASM entry point (raycore-wasm)
│   └── src/
│       ├── lib.rs              # #[wasm_bindgen] exports, event handling, render loop
│       └── canvas_manager.rs   # Multi-canvas layout manager
├── shaders/                    # WGSL GPU shaders
│   ├── rect.wgsl               # Instanced rectangle shader (candles + wicks)
│   ├── volume.wgsl             # Volume bar shader
│   ├── candle.wgsl             # Legacy candle shader
│   └── candles.wgsl            # Legacy candle shader
├── demo/                       # Browser demo
│   ├── index.html              # Demo page with live chart
│   └── pkg/                    # Compiled WASM + JS glue
├── Cargo.toml                  # Workspace root
└── LICENSE                     # Proprietary license
```

---

## Rendering Pipeline

1. **Data ingestion** — OHLCV JSON is parsed into `CandleData` structs and loaded into `ChartEngine`.
2. **Viewport calculation** — Visible candle range, price bounds, and pixel mapping are computed based on zoom/pan state.
3. **GPU instance buffers** — Candle bodies, wicks, and volume bars are packed into per-instance vertex buffers (`bytemuck`).
4. **Instanced draw calls** — A single `rect.wgsl` shader draws all visible candles in one GPU draw call. Volume bars use `volume.wgsl`.
5. **Canvas 2D overlays** — Price axis, time axis, and crosshair are rendered on separate Canvas 2D layers stacked above the WebGPU surface.
6. **requestAnimationFrame** — The render loop runs at display refresh rate, only re-rendering when state changes (dirty flag).

---

## Key Technical Decisions

| Decision | Rationale |
|---|---|
| **WebGPU over WebGL** | Modern API, compute shaders, better instancing, future-proof |
| **Rust + WASM** | Zero-cost abstractions, memory safety, native perf in browser |
| **Instanced rendering** | One draw call for thousands of candles instead of one per candle |
| **Multi-canvas layering** | Separates GPU surface from 2D text overlays — avoids readback |
| **No framework dependency** | Pure Rust + raw DOM — zero JS runtime overhead |

---

## Build

### Prerequisites

- Rust 1.83+ with `wasm32-unknown-unknown` target
- [wasm-pack](https://rustwasm.github.io/wasm-pack/)

### Compile to WASM

```bash
wasm-pack build wasm --target web --out-dir ../pkg --release
```

### Run demo

```bash
# Copy built artifacts to demo/pkg/, then serve:
python serve.py
# Open http://localhost:8080/demo/
```

---

## Interaction

- **Scroll** — Zoom in/out (horizontal scale)
- **Click + Drag** — Pan through time
- **Mouse move** — Crosshair with price/time tooltip

---

## Performance

- **10,000+ candles** rendered in a single instanced draw call
- **Sub-millisecond frame times** on integrated GPUs
- **Zero GC pressure** — no JavaScript objects created per frame
- **Dirty-flag rendering** — skips frames when nothing changes

---

## License

**Proprietary. All rights reserved.**

This software is the exclusive intellectual property of [RayCharts](https://raycharts.com/). You may NOT copy, use, modify, distribute, or create derivative works from this software without prior written authorization. See [LICENSE](./LICENSE) for full terms.

---

*Built by [RayCharts](https://raycharts.com/)*
