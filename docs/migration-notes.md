# Migration Notes

This document tracks the `migration/f64-price-domain-and-hardening` pass.

## Verification Summary

Verified against code commit `97ae55b` on 2026-04-17.

Commands completed successfully:

- `cargo check`
- `cargo check --target wasm32-unknown-unknown -p axiuscharts-wasm`
- `cargo test`
- `cargo test --features parity-tests`
- `cargo clippy -- -D warnings`
- `wasm-pack build wasm --target web --release`
- `cargo bench --bench core_benchmarks`

Produced code commits:

- `a7738ae` `feat(data): migrate logical price domain to f64`
- `261fb23` `feat(core): emit glide events and migrate drawing snapshots`
- `97ae55b` `test(renderer): add backend parity harness`

## What Changed

### Issue 1 and 2: logical prices to `f64`, remove `Bar::_pad`

- `Bar` prices and volume moved to `f64`
- `Bar::_pad` was removed
- `Bar::new(timestamp, open, high, low, close, volume)` was added
- `BarArray` moved to `Float64Array` / `Float64Builder`
- logical-domain series data, footprint levels, study outputs, execution mark prices, and WASM typed-array inputs were migrated to `f64`
- renderer submission still converts only projected render-space values to single precision at the seam

### Issue 3: stale feature audit removed

- Deleted `feature_audit.md.resolved`
- Replaced it with `docs/feature-matrix.md`

### Issue 4: remaining snake_case doc audit

- Re-audited repo docs for public JS event payload naming
- Kept public event payload docs in camelCase
- Checked the remaining alignment item in `ALIGNMENT_TODO.md`

### Issue 5: kinetic glide emits `visibleRangeChange`

- Glide frames now emit `visibleRangeChange` when the visible range changes
- Emission stays throttled to at most once per RAF frame
- Added `glide_tick_emits_visible_range_change_while_animation_is_active`
- Checked the glide item in `TODO.md`

### Issue 6: drawing snapshot migration framework

- Added `DrawingsMigrationError`
- Added `migrate_snapshot(payload: &serde_json::Value)`
- Routed drawing import through the migration entry point
- Added round-trip, unknown-version, and future-version tests

### Issue 7: backend parity scaffold

- Added the `parity-tests` feature
- Added a native-only structural parity harness and integration test
- `cargo test --features parity-tests` now generates `target/backend-parity-report.md`

### Issue 8: Worker offload plan

- Documented the indicator Worker offload seam in `docs/indicator-runtime.md`

## Breaking Changes

- Public logical price paths moved from `f32` to `f64` at the Rust and WASM boundaries
- `Bar::_pad` was removed
- `feature_audit.md.resolved` was deleted
- `docs/persistent.md` was replaced by `docs/drawing-persistence.md`

## Behavioral Changes

- `visibleRangeChange` now fires during kinetic glide, not just after it settles
- Drawing imports now go through version-aware migration instead of direct deserialize-only loading
- Logical prices round-trip through storage and persistence without render-precision truncation

## New Public Items

- `Bar::new(...)`
- `migrate_snapshot(...)`
- `DrawingsMigrationError`
- existing `DRAWINGS_SNAPSHOT_VERSION` remains re-exported and is now backed by a migration entry point

## Dependency Changes

- No dependency version bump was required
- `arrow = 55.2.0` already provided the `Float64Builder` path needed for the migration
- No new runtime dependencies were added

## Pre-Migration Baseline

- Baseline branch point: `8afae962df1c10c18cc1fa841bb15243fc644ab9`
- Capture date: 2026-04-17
- Baseline commit on this branch: `4e1c096`
- Commands:
  - `cargo bench --bench core_benchmarks`
  - `wasm-pack build wasm --target web --release`
- Pre-migration WASM artifact:
  - `wasm/pkg/axiuscharts_wasm_bg.wasm`: `2,110,177` bytes

### Pre-migration benchmark medians

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

## Post-Migration Results

### WASM Artifact Size

- Post-migration artifact: `wasm/pkg/axiuscharts_wasm_bg.wasm`
- Size: `2,108,341` bytes
- Delta vs baseline: `-1,836` bytes (`-0.087%`)

### Post-migration benchmark medians

| Benchmark | Median |
| --- | --- |
| `BarArray::set/100` | `4.6100 µs` |
| `BarArray::set/1000` | `25.824 µs` |
| `BarArray::set/10000` | `243.24 µs` |
| `BarArray::set/100000` | `3.9631 ms` |
| `BarArray::append/initial_size/0` | `180.86 ns` |
| `BarArray::append/initial_size/100` | `103.54 ns` |
| `BarArray::append/initial_size/1000` | `117.60 ns` |
| `BarArray::append/initial_size/10000` | `170.42 ns` |
| `BarArray::append_streaming/1000_bars_streamed` | `178.03 µs` |
| `BarArray::access/get_checked` | `17.100 µs` |
| `BarArray::access/get_unchecked` | `15.736 µs` |
| `BarArray::access/direct_accessor_close` | `7.5729 µs` |
| `BarArray::update_last/100` | `3.4119 µs` |
| `BarArray::update_last/1000` | `19.148 µs` |
| `BarArray::update_last/10000` | `201.95 µs` |
| `Viewport::transforms/bar_to_frac` | `1.9984 µs` |
| `Viewport::transforms/price_to_frac` | `2.0189 µs` |
| `Viewport::transforms/price_to_css_y` | `2.1215 µs` |
| `Viewport::transforms/pixel_to_bar` | `2.0556 µs` |
| `Viewport::transforms/bar_index_at_pixel` | `2.9318 µs` |
| `Viewport::auto_fit_price/100` | `179.58 ns` |
| `Viewport::auto_fit_price/1000` | `1.6332 µs` |
| `Viewport::auto_fit_price/10000` | `16.173 µs` |
| `Viewport::zoom/zoom_in` | `19.464 ns` |
| `Viewport::zoom/zoom_out` | `19.648 ns` |
| `Viewport::zoom/rapid_zoom_sequence` | `107.28 ns` |
| `Viewport::pan/pan_simple` | `23.069 ns` |
| `Viewport::pan/pan_clamped` | `28.234 ns` |
| `Viewport::pan/rapid_pan_sequence` | `60.334 ns` |

### Highlighted benchmark deltas

- `BarArray::set/100000`: `2.7417 ms -> 3.9631 ms` (`+44.55%`)
- `BarArray::append/initial_size/10000`: `392.69 ns -> 170.42 ns` (`-56.60%`)
- `BarArray::access/direct_accessor_close`: `7.7126 µs -> 7.5729 µs` (`-1.81%`)
- `BarArray::update_last/10000`: `175.60 µs -> 201.95 µs` (`+15.01%`)
- `Viewport::auto_fit_price/10000`: `24.924 µs -> 16.173 µs` (`-35.11%`)
- `Viewport::zoom/rapid_zoom_sequence`: `200.62 ns -> 107.28 ns` (`-46.53%`)

## Deferred Items

- None
