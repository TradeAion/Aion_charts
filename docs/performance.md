# Performance

AxiusCharts performance comes from three main choices: columnar storage, invalidation-driven rendering, and backend-specific geometry emission that avoids rebuilding more state than necessary.

## Architectural Notes

- `BarArray` uses Arrow-backed columnar arrays for cache-friendly scans and append-heavy updates.
- Rendering is invalidation-driven, not a permanent RAF loop, which reduces idle work and battery burn.
- Geometry is built from logical-domain doubles and converted into renderer-friendly attributes at the final seam.
- GPU work is batched around shared geometry generation and draw-list submission.

## Benchmarks

Representative post-migration medians from `cargo bench --bench core_benchmarks`:

| Benchmark | Median |
| --- | --- |
| `BarArray::set/100` | `4.6100 µs` |
| `BarArray::set/100000` | `3.9631 ms` |
| `BarArray::append/initial_size/10000` | `170.42 ns` |
| `BarArray::access/direct_accessor_close` | `7.5729 µs` |
| `BarArray::update_last/10000` | `201.95 µs` |
| `Viewport::auto_fit_price/10000` | `16.173 µs` |
| `Viewport::zoom/rapid_zoom_sequence` | `107.28 ns` |

Compared with the pre-migration baseline captured at the start of this branch:

- `BarArray::set/100000`: `2.7417 ms -> 3.9631 ms` (`+44.55%`)
- `BarArray::append/initial_size/10000`: `392.69 ns -> 170.42 ns` (`-56.60%`)
- `BarArray::access/direct_accessor_close`: `7.7126 µs -> 7.5729 µs` (`-1.81%`)
- `BarArray::update_last/10000`: `175.60 µs -> 201.95 µs` (`+15.01%`)
- `Viewport::auto_fit_price/10000`: `24.924 µs -> 16.173 µs` (`-35.11%`)
- `Viewport::zoom/rapid_zoom_sequence`: `200.62 ns -> 107.28 ns` (`-46.53%`)

## Render Scheduling Tradeoff

Invalidation-driven rendering wins when the chart is idle or mostly static. A permanent RAF loop would simplify some animation code, but it would also waste frames when nothing changed. AxiusCharts keeps one-shot scheduling and explicitly re-queues during glide, replay, and pane animation instead.

## Storage And Streaming

- Columnar arrays keep scans fast for auto-fit, indicators, and label generation.
- Append and update-last paths are designed for live feeds.
- Pending-buffer append patterns and auto-flush thresholds matter most once ingestion becomes bursty rather than interactive.

## GPU Strategy

- Shared geometry generators feed both render backends.
- The WebGPU path benefits from compact attribute buffers after viewport projection.
- The Canvas2D path keeps logical-domain `number` values until draw time.

## Known Bottlenecks

- Large full-history auto-fit scans become noticeable before append/update paths do.
- Complex indicator workloads can contend with rendering on the main thread.
- Pixel-perfect backend parity is not automated yet; current parity coverage is structural.

## WASM Bundle Size

Release artifact size after the migration:

- `wasm/pkg/axiuscharts_wasm_bg.wasm`: `2,108,341` bytes
- Baseline: `2,110,177` bytes
- Delta: `-1,836` bytes (`-0.087%`)

The full capture and command log are recorded in [migration-notes.md](./migration-notes.md).
