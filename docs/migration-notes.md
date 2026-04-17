# Migration Notes

This document tracks the `migration/f64-price-domain-and-hardening` pass.

## Pre-Migration Baseline

- Baseline branch point: `8afae962df1c10c18cc1fa841bb15243fc644ab9`
- Capture date: 2026-04-17
- Commands:
  - `cargo bench --bench core_benchmarks`
  - `wasm-pack build wasm --target web --release`
- Pre-migration WASM artifact:
  - `wasm/pkg/axiuscharts_wasm_bg.wasm`: `2,110,177` bytes

### Benchmark medians

| Benchmark | Median |
| --- | --- |
| `BarArray::set/100` | `4.7465 µs` |
| `BarArray::set/1000` | `24.665 µs` |
| `BarArray::set/10000` | `231.23 µs` |
| `BarArray::set/100000` | `2.7417 ms` |
| `BarArray::append/initial_size/0` | `182.57 ns` |
| `BarArray::append/initial_size/100` | `215.53 ns` |
| `BarArray::append/initial_size/1000` | `162.68 ns` |
| `BarArray::append/initial_size/10000` | `392.69 ns` |
| `BarArray::append_streaming/1000_bars_streamed` | `188.55 µs` |
| `BarArray::access/get_checked` | `17.979 µs` |
| `BarArray::access/get_unchecked` | `11.911 µs` |
| `BarArray::access/direct_accessor_close` | `7.7126 µs` |
| `BarArray::update_last/100` | `4.6022 µs` |
| `BarArray::update_last/1000` | `19.408 µs` |
| `BarArray::update_last/10000` | `175.60 µs` |
| `Viewport::transforms/bar_to_frac` | `2.0310 µs` |
| `Viewport::transforms/price_to_frac` | `2.0407 µs` |
| `Viewport::transforms/price_to_css_y` | `2.0874 µs` |
| `Viewport::transforms/pixel_to_bar` | `2.0616 µs` |
| `Viewport::transforms/bar_index_at_pixel` | `2.9193 µs` |
| `Viewport::auto_fit_price/100` | `286.44 ns` |
| `Viewport::auto_fit_price/1000` | `2.5259 µs` |
| `Viewport::auto_fit_price/10000` | `24.924 µs` |
| `Viewport::zoom/zoom_in` | `28.767 ns` |
| `Viewport::zoom/zoom_out` | `28.712 ns` |
| `Viewport::zoom/rapid_zoom_sequence` | `200.62 ns` |
| `Viewport::pan/pan_simple` | `27.805 ns` |
| `Viewport::pan/pan_clamped` | `27.184 ns` |
| `Viewport::pan/rapid_pan_sequence` | `61.569 ns` |

### Notes

- The pre-migration benchmark target emits warnings in `benches/core_benchmarks.rs` for ignored `Result` values and one unused loop variable. These are existing issues and will be resolved as part of the migration so `cargo clippy -- -D warnings` passes cleanly.
- The final sections of this document will be filled in after the migration is complete.
