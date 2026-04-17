# API Reference

This document summarizes the public WASM API. The hand-maintained TypeScript contract in [`../wasm/axiuscharts.d.ts`](../wasm/axiuscharts.d.ts) is the exact source of truth for signatures and payload types.

## Lifecycle

### `AxiusCharts.create_chart(container, options)`

Creates a chart against an `HTMLElement` or a bare element ID string.

- `renderer`: `"auto"`, `"webgpu"`, or `"canvas2d"`
- `autoRender`: invalidation-driven one-shot RAF scheduling
- `theme`: `"dark"`, `"light"`, or a custom theme object
- `symbol`, `interval`, `crosshair`, `priceScale`

### `apply_options(options)`

Applies runtime-safe option changes. `renderer` is create-time only; `theme`, `symbol`, `interval`, `crosshair`, `priceScale`, and `autoRender` are mutable.

### `render()`, `start_auto_render()`, `stop_auto_render()`, `is_auto_render()`, `dispose()`

- `render()` renders one frame immediately
- `start_auto_render()` and `stop_auto_render()` control invalidation-driven scheduling
- `is_auto_render()` reports current scheduling mode
- `dispose()` tears down listeners, RAF state, and DOM/WASM resources

## Data And Precision

### Main bars

```ts
chart.set_data_arrays(
  open: Float64Array,
  high: Float64Array,
  low: Float64Array,
  close: Float64Array,
  volume: Float64Array,
  timestamps: BigUint64Array,
);
```

- Prices and volumes are logical-domain doubles end-to-end.
- `append_bar`, `update_last_bar`, and `upsert_bar` also take `number` values in the same logical domain.
- Validation rejects non-finite writes instead of silently coercing them.

### Footprint bars

```ts
chart.set_data_with_footprint_arrays(
  open: Float64Array,
  high: Float64Array,
  low: Float64Array,
  close: Float64Array,
  volume: Float64Array,
  timestamps: BigUint64Array,
  levelOffsets: Uint32Array,
  prices: Float64Array,
  bidVolumes: Float64Array,
  askVolumes: Float64Array,
);
```

Use this path for canonical historical footprint loads. `set_footprint_bar(...)`, `set_footprint_data_arrays(...)`, and `set_footprint_data_json(...)` remain available for compatibility and patch workflows.

### Overlay series

- `add_line_series`, `add_area_series`, `add_histogram_series`, `add_bar_series`, `add_baseline_series`
- `set_series_data(...)` uses `Float64Array` values
- `set_histogram_data(...)` uses `Float64Array` values plus per-point color arrays
- `set_bar_series_data(...)` uses `Float64Array` OHLC arrays
- Streaming helpers (`append_*`, `update_last_*`, `upsert_*`) preserve the same logical `number` domain

### Studies

- `create_study(...)` supports `sma`, `ema`, `rsi`, `macd`, `bollinger`, `stochastic`, `atr`, `vwap`
- `set_study_parameter(...)` accepts `number`
- `get_study_output(...)` returns `{ timestamps: BigUint64Array, values: Float64Array }`

See [price-domain.md](./price-domain.md) for the precision contract.

## Viewport And Interaction

- `visible_range()` returns `[startBar, endBar]`
- `set_visible_range(start, end)` updates the logical bar window
- `zoom_to_range(startTimestampMs, endTimestampMs)` zooms from timestamps
- `reset_viewport(mode?)` accepts `"default"` and `"fit_all"`
- `set_auto_scroll(enabled)` and `get_auto_scroll()` control live scrolling
- `set_crosshair_mode(mode)` accepts `"normal"` and `"magnet_ohlc"`

`visibleRangeChange` is emitted during drag, wheel, pinch, reset, and kinetic glide, throttled to at most once per RAF frame.

## Main Chart Types

`set_chart_type(...)` accepts:

- `candlestick`
- `ohlc`
- `line`
- `area`
- `heikin_ashi`
- `baseline`
- `footprint`

Aliases such as `candles`, `bars`, `ha`, `fp`, and `order_flow` are preserved.

## Drawing, Markers, And Execution Marks

- Drawings: `set_drawing_tool`, `cancel_drawing`, `remove_selected_drawing`, `clear_drawings`, `remove_all_scale_drawings`
- Persistence: `export_drawings`, `import_drawings`, `export_persistence_state`, `import_persistence_state`
- Price lines: `create_price_line`, `set_price_line_price`, `set_price_line_label`, `set_price_line_visible`, `remove_price_line`
- Markers: `add_marker`, `set_markers`, `remove_marker`, `clear_markers`, `clear_all_markers`
- Execution marks: `add_execution_mark`, `add_execution_mark_full`, `set_execution_marks`, `set_execution_marks_json`, `get_execution_marks_json`, `set_execution_label_mode`, `get_execution_label_mode`, `set_execution_pnl_visible`, `get_execution_pnl_visible`, `set_execution_cluster_threshold_px`, `expand_execution_cluster`

Execution mark prices are `number` values in the same logical-domain double-precision price space as bars and studies.
Execution-mark JSON snapshots now serialize as `{ version, marks }`, while legacy bare arrays remain accepted on import.

See [execution-marks.md](./execution-marks.md) and [execution-marks-persistence.md](./execution-marks-persistence.md) for the execution-specific contract.

## Indicators And Panes

- Indicator VM: `indicator_compile`, `indicator_attach`, `indicator_detach`, `indicator_set_inputs`, `indicator_set_mtf_snapshot`, `indicator_set_enabled`
- Diagnostics: `indicator_list`, `indicator_get_diagnostics`, `indicator_get_mtf_requests`, `indicator_get_stats`, `indicator_drain_events`
- Panes: `add_indicator_pane`, `remove_indicator_pane`, `update_indicator_pane`, `indicator_pane_count`, `drag_pane_separator`

See [indicator-runtime.md](./indicator-runtime.md) for the runtime model and the planned Worker offload seam.

## Events

All public JS event payloads use camelCase field names. See [events.md](./events.md) for the complete event catalog and payload tables.
