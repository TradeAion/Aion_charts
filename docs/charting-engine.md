# Charting Engine

AxiusCharts is the charting engine itself, not a wrapper around a third-party plotting library. The Rust core owns data structures, viewport math, geometry generation, render backend selection, drawings, studies, indicators, and event delivery.

## What The Engine Is

- A platform-agnostic Rust core that models chart state and rendering inputs.
- A WASM-facing API that exposes that core to JavaScript with typed events.
- A renderer stack with WebGPU primary and Canvas2D fallback backends.
- A trading-focused interaction model that includes replay, execution marks, footprint charts, and drawing tools.

## What The Engine Is Not

- It is not a thin shell around an external charting vendor.
- It is not a permanent RAF-driven animation loop.
- It is not a browser-only precision model; the logical domain is owned by the Rust core.

## Public API Surface

The stable public names include:

- `create_chart`
- `apply_options`
- `on`, `off`, `once`
- `start_auto_render`, `stop_auto_render`, `is_auto_render`
- `render`
- `dispose`
- `get_css_variables`
- `theme`

See [api-reference.md](./api-reference.md) and [`../wasm/axiuscharts.d.ts`](../wasm/axiuscharts.d.ts) for the full surface.

## Renderer Selection

- `webgpu`: prefer WebGPU, fall back to Canvas2D if initialization fails
- `auto`: same as `webgpu`
- `canvas2d`: force Canvas2D directly

WebGPU is the primary path because the engine is built around shared geometry generation and batched GPU-friendly draw submission. Canvas2D remains the compatibility backend.

## Browser Support Expectations

| Environment | Status |
| --- | --- |
| Chromium browsers with WebGPU enabled | Preferred path |
| Browsers without WebGPU | Canvas2D fallback |
| Browsers with experimental WebGPU support | Supported when WebGPU init succeeds, otherwise fallback |

## Product Boundary

There is no "vendor fallback library" behind AxiusCharts. The engine itself is the product boundary, which is why data precision, event semantics, persistence versioning, and backend parity are maintained in-repo instead of delegated elsewhere.
