# API Reference

Complete reference for the RayCore WASM public API.

---

## Lifecycle

### `RayCore.create_chart(container, options)`

Create a new chart instance.

```ts
static create_chart(
  container: HTMLElement | string,
  options?: CreateChartOptions
): Promise<RayCore>
```

**Parameters:**
- `container` — DOM element or element ID string
- `options.renderer` — `'webgpu'` (default), `'auto'`, or `'canvas2d'`
- `options.autoRender` — `true` to start RAF loop automatically
- `options.theme` — `'dark'`, `'light'`
- `options.symbol` — symbol string (e.g. `'BTCUSD'`)
- `options.interval` — interval string (e.g. `'1m'`, `'1h'`, `'1D'`)

Renderer resolution:
- `'webgpu'`: attempts WebGPU first, then falls back to Canvas2D on failure
- `'auto'`: same preference as `'webgpu'` (WebGPU-first with fallback)
- `'canvas2d'`: always uses Canvas2D directly

### `apply_options(options)`

Update chart options at runtime.

```ts
apply_options(options: { theme?: 'dark' | 'light' }): void
```

Notes:
- `renderer` is create-time only.
- `apply_options({ renderer: ... })` is ignored and emits an `error` event.

### `dispose()`

Detach all event listeners, stop RAF loop, and free WASM memory. Always call this when removing the chart.

### `render()`

Render a single frame. Only needed if `autoRender` is `false`.

### `start_auto_render()` / `stop_auto_render()`

Control the internal `requestAnimationFrame` loop.

---

## Data

### `set_data_arrays(open, high, low, close, volume, timestamps)`

Load OHLCV bar data from parallel typed arrays.

```ts
set_data_arrays(
  open: Float32Array,
  high: Float32Array,
  low: Float32Array,
  close: Float32Array,
  volume: Float32Array,
  timestamps: BigUint64Array  // millisecond unix timestamps
): void
```

### `upsert_bar(timestamp, open, high, low, close, volume)`

Insert or update a single bar. If a bar with the given timestamp exists, it is updated; otherwise a new bar is appended.

### `append_bar(timestamp, open, high, low, close, volume)`

Append a new bar (must have a timestamp greater than the last bar).

### `update_last_bar(open, high, low, close, volume)`

Update the most recent bar in-place (for live streaming).

### `demo_mode()`

Load 600 sample bars for testing.

---

## Replay

### `set_replay_mode(enabled)` / `replay_mode()`

Enter or exit market replay mode.

- Entering replay snapshots current bars into an internal archive.
- Exiting replay restores the full archived timeline and jumps back to latest/live bars.
- Replay state is runtime-only (not exported in persistence snapshots).

### `set_replay_playing(playing)` / `replay_playing()`

Start/pause frame-driven replay playback.

- Playback speed is configured by `ReplayOptions.speedBarsPerSecond`.
- In charts created with `autoRender: false`, starting replay playback temporarily enables RAF rendering; pausing/exiting replay restores manual mode.

### `replay_step_back()` / `replay_step_forward()`

Move replay cutoff by exactly 1 bar backward/forward.

### `set_replay_cutoff_bar(index)` / `replay_cutoff_bar()`

Set/get replay right-edge cutoff (inclusive index in the archived timeline).

- `replay_cutoff_bar()` returns `-1` when unavailable (for example, empty data).
- Clicking the main chart pane in replay mode also sets this cutoff.

### `set_replay_options(options)` / `replay_options()`

```ts
type ReplayEdgeBehavior = 'auto_pause' | 'live_continue' | 'auto_exit';

interface ReplayOptions {
  speedBarsPerSecond?: number; // default: 1.0
  edgeBehavior?: ReplayEdgeBehavior; // default: 'auto_pause'
}
```

Edge behavior:
- `auto_pause`: pause at the replay edge
- `live_continue`: stay playing at edge and continue when buffered live bars arrive
- `auto_exit`: exit replay mode and jump to live when edge is reached

---

## Viewport

### `zoom_to_range(start, end)`

Zoom to a specific timestamp range (millisecond unix timestamps).

### `set_visible_range(start, end)` / `visible_range()`

Get or set the currently visible timestamp range.

### `data_range()`

Returns the full data timestamp range `[start, end]`.

### `set_auto_scroll(enabled)` / `get_auto_scroll()`

Toggle auto-scroll to latest bar on new data.

---

## Chart Type

### `set_chart_type(type)`

```ts
set_chart_type(type: 'candlestick' | 'ohlc' | 'line' | 'area' | 'heikin_ashi' | 'baseline'): void
```

Aliases: `'candles'` = `'candlestick'`, `'bars'` = `'ohlc'`, `'ha'` = `'heikin_ashi'`.

---

## Price Scale

### `set_price_scale_mode(mode)`

```ts
set_price_scale_mode(mode: 'normal' | 'logarithmic' | 'percentage' | 'indexed_to_100'): void
```

### `set_price_scale_margins(top, bottom)`

Set top/bottom margins as fractions (0.0-1.0).

---

## Crosshair

### `set_crosshair_mode(mode)`

```ts
set_crosshair_mode(mode: 'normal' | 'magnet_ohlc'): void
```

See [Theming](./theming.md) for crosshair styling methods.

---

## Drawing Tools

### `set_drawing_tool(tool)`

```ts
set_drawing_tool(tool:
  | 'none'
  | 'trend_line'
  | 'horizontal_line'
  | 'vertical_line'
  | 'ray'
  | 'rectangle'
  | 'fibonacci'
  | 'scale'
  | 'brush'
): void
```

See [Drawing Tools](./drawing-tools.md) for detailed usage.

### `remove_selected_drawing()` / `cancel_drawing()` / `clear_drawings()`

Drawing lifecycle management.

### `export_persistence_state(layoutId?)` / `import_persistence_state(json)`

Persist and restore full chart state (recommended):

- Drawings
- Chart styles/theme options
- Viewport state
- Indicator pane layout
- Volume visibility/colors and price-scale tick visuals (`ticksVisible`, `tickDensity`)

```ts
const snapshot = chart.export_persistence_state('workspace-main');
localStorage.setItem('raycore.chart-state', snapshot);

const saved = localStorage.getItem('raycore.chart-state');
if (saved) chart.import_persistence_state(saved);
```

### `export_drawings()` / `import_drawings(json)`

Persist and restore drawings only (legacy/partial snapshot).

- Restore is atomic for existing panes: invalid payloads fail without clearing current drawings.

```ts
const snapshot = chart.export_drawings();
localStorage.setItem('raycore.drawings', snapshot);

const saved = localStorage.getItem('raycore.drawings');
if (saved) chart.import_drawings(saved);
```

See [Persistent State Guide](./persistent.md) for full production guidance.

---

## Series Overlays

### `add_line_series(r, g, b, a, width, style, timestamps, values)`

Add a line series overlay. Returns a `series_id` (number).

### `add_area_series(...)` / `add_bar_series(...)` / `add_baseline_series(...)` / `add_histogram_series(...)`

Add other series types. See TypeScript definitions for full parameter lists.

### `remove_series(id)` / `set_series_visible(id, visible)` / `series_count()`

Manage overlay series.

### `set_series_data(id, values, timestamps)` / `upsert_series_point(id, timestamp, value)`

Update series data.

---

## Studies (Technical Indicators)

### `create_study(type)`

```ts
create_study(type: 'sma' | 'ema' | 'rsi' | 'macd' | 'bollinger' | 'stochastic' | 'atr' | 'vwap'): number
```

Returns a `study_id`.

### `remove_study(id)` / `set_study_parameter(id, key, value)` / `study_count()`

Manage studies.

### `add_indicator_pane(study_id, type, height)` / `remove_indicator_pane(pane_id)`

Add sub-chart panes for indicators like RSI, MACD, etc.

---

## Price Lines

### `create_price_line(price, r, g, b, a, width, style, label)`

Create a horizontal price line. Returns a `price_line_id`.

### `remove_price_line(id)` / `set_price_line_price(id, price)` / `set_price_line_visible(id, visible)`

Manage price lines.

---

## Series Markers

### `add_marker(series_id, timestamp, position, shape, r, g, b, a, text)`

Add a marker (arrow, circle, square) at a specific bar.

- `position`: `'above_bar'`, `'below_bar'`, `'at_price'`
- `shape`: `'arrow_up'`, `'arrow_down'`, `'circle'`, `'square'`

### `remove_marker(series_id, marker_id)` / `clear_markers(series_id)` / `clear_all_markers()`

---

## Events

### `on(event, callback)` / `off(event, callback)` / `once(event, callback)`

Subscribe to chart events:

| Event | Payload |
|---|---|
| `crosshairMove` | `{ price, timestamp, bar_index, x, y }` |
| `click` | `{ price, timestamp, bar_index, x, y }` |
| `visibleRangeChange` | `{ start, end }` |
| `symbolChange` | `{ symbol }` |
| `intervalChange` | `{ interval }` |
| `chartTypeChange` | `{ chart_type }` |
| `priceScaleChange` | `{ mode }` |
| `resize` | `{ width, height }` |
| `drawingCreated` | `{ id, tool }` |
| `drawingSelected` | `{ id }` |
| `rendererFallback` | `{ requested, active, reason }` |
| `error` | `{ message }` |

---

## Multi-Chart

### `ChartGroup`

Synchronize multiple chart panes:

```ts
const group = new ChartGroup();
group.add_pane(chart1_id, 'BTCUSD', '1h');
group.add_pane(chart2_id, 'ETHUSD', '1h');
group.link_panes(chart1_id, chart2_id, 'link-1');
group.set_sync('link-1', 'time', true);
group.set_sync('link-1', 'crosshair', true);
```

### `ChartWorkspace`

Split-pane layout management:

```ts
const ws = new ChartWorkspace('container-id');
ws.split_active('vertical');
```

---

## Static Methods

### `RayCore.get_supported_renderers()`

Returns `['webgpu', 'canvas2d']` or `['canvas2d']` depending on browser support.

### `chart.renderer_name()`

Returns the active renderer name (`'webgpu'` or `'canvas2d'`).
