# Price Domain

AxiusCharts uses a two-domain price architecture:

- Logical price domain: `f64`
- Render attribute domain: single-precision screen-space values

This separation is deliberate. Crypto prices like `103842.5712345` and micro-priced assets like `0.00000012345678` need double precision in storage and math, while GPU vertex attributes are still most efficient in single precision after the viewport origin has been subtracted.

## Why Logical Prices Use `f64`

- `f32` cannot preserve both large nominal prices and fine-grained ticks at the same time.
- Bars, overlays, studies, crosshair readouts, drawings, price lines, and execution marks all share the same logical price domain.
- The WASM boundary maps naturally onto JavaScript `number`, so moving logical prices to `f64` removes needless conversion loss.
- Arrow-backed columnar storage now uses `Float64Array` / `Float64Builder`.

## Why Render Attributes Stay Single Precision

- WebGPU vertex attributes and WGSL input layouts are already single-precision.
- Renderer bandwidth and buffer footprint matter more than sub-tick world precision once values are projected into pane-local screen space.
- Canvas2D already accepts JS `number`, so the downcast is mostly relevant for GPU vertex buffers and uniforms.

## The Seam

The seam is the projection layer. Logical prices remain `f64` until the point where they have been converted into pane-local coordinates.

```rust
use axiuscharts::core::renderer::value_projection::price_to_pane_y_phys;

fn project_price_for_gpu(price: f64, viewport: &Viewport, pane_height: f64) -> f32 {
    // SEAM: logical f64 -> single-precision render-space
    price_to_pane_y_phys(price, viewport, pane_height) as f32
}
```

Do not cast directly from a raw logical value into a vertex attribute without going through projection.

## What Not To Do

- Do not store logical prices in `f32`.
- Do not serialize drawing anchors in `f32`.
- Do not cast `bar.close as f32` or `series.value as f32` outside the projection seam.
- Do not introduce `Float32Array` typed-array inputs for logical prices at the WASM boundary.

## Round-Trip Guarantee

A price written into the logical domain and read back from the logical domain is preserved exactly. In practice:

- `set_data_arrays(...)` accepts `Float64Array`
- `BarArray::get(...)` returns `f64`
- drawing snapshots keep price anchors as `f64`
- study outputs return `Float64Array`

## Regression Examples

```rust
use axiuscharts::{Bar, BarArray};

#[test]
fn bar_preserves_crypto_precision() {
    let bar = Bar::new(
        1_700_000_000_000,
        103_842.57,
        103_842.58,
        103_842.56,
        103_842.5712345,
        1_000.0,
    );
    assert_eq!(bar.close, 103_842.5712345);

    let mut arr = BarArray::new();
    arr.set(vec![bar]).unwrap();
    assert_eq!(arr.get(0).unwrap().close, 103_842.5712345);
}

#[test]
fn bar_preserves_small_alt_precision() {
    let bar = Bar::new(
        1_700_000_000_000,
        0.0000001234,
        0.0000001235,
        0.0000001233,
        0.00000012345678,
        1.0,
    );

    let mut arr = BarArray::new();
    arr.set(vec![bar]).unwrap();
    assert_eq!(arr.get(0).unwrap().close, 0.00000012345678);
}
```

These are not illustrative examples only; equivalent regression tests live in `src/core/data.rs`.
