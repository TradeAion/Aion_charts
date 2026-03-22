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

### `set_data_with_footprint_arrays(open, high, low, close, volume, timestamps, level_offsets, prices, bid_volumes, ask_volumes)`

Atomically load OHLCV bars plus aligned footprint levels from typed arrays.

```ts
set_data_with_footprint_arrays(
  open: Float32Array,
  high: Float32Array,
  low: Float32Array,
  close: Float32Array,
  volume: Float32Array,
  timestamps: BigUint64Array,
  level_offsets: Uint32Array,
  prices: Float32Array,
  bid_volumes: Float32Array,
  ask_volumes: Float32Array,
): void
```

Notes:
- This is the canonical historical footprint initialization API for production use.
- `level_offsets.length` must equal `bar_count + 1`.
- Sparse footprint bars use empty ranges, for example `level_offsets[i] === level_offsets[i + 1]`.
- Validation is atomic: invalid payloads fail without partially replacing chart state.

### `set_data_with_footprint_json(json)`

Atomically load OHLCV bars plus footprint levels from JSON.

Canonical format:

```json
[
  {
    "timestamp": 1710000000000,
    "open": 100.0,
    "high": 101.0,
    "low": 99.5,
    "close": 100.5,
    "volume": 2500.0,
    "levels": [
      { "price": 99.5, "bid": 120.0, "ask": 80.0 },
      { "price": 100.0, "bidVolume": 90.0, "askVolume": 140.0 }
    ]
  }
]
```

Also accepted: `{ "bars": [...] }`.

### `upsert_bar_with_footprint(timestamp, open, high, low, close, volume, prices, bid_volumes, ask_volumes)`

Atomically append/update a live OHLCV bar and its footprint levels in one call.

This is the canonical live-update API for production footprint integrations.

### Legacy compatibility footprint setters

The following methods remain supported for patch/update workflows and backward compatibility:

- `set_footprint_bar(...)`
- `set_footprint_data_arrays(...)`
- `set_footprint_data_json(...)`

For new historical footprint initialization, prefer `set_data_with_footprint_arrays(...)` or `set_data_with_footprint_json(...)`.

### Production example

```ts
chart.set_chart_type('footprint');
chart.set_data_with_footprint_arrays(
  opens,
  highs,
  lows,
  closes,
  volumes,
  timestamps,
  levelOffsets,
  prices,
  bidVolumes,
  askVolumes,
);
```

### Compatibility example

```ts
chart.set_data_arrays(opens, highs, lows, closes, volumes, timestamps);
chart.set_chart_type('footprint');
chart.set_footprint_data_arrays(barIndices, levelOffsets, prices, bidVolumes, askVolumes);
```

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

### `reset_viewport(mode?)`

Reset the main chart viewport using one of two presets:

- `default`: restore the recent-bars default view with a small right-side gap
- `fit_all`: show the full dataset with a small right-side gap

Unknown or omitted modes fall back to `default`.

### `data_range()`

Returns the full data timestamp range `[start, end]`.

### `set_auto_scroll(enabled)` / `get_auto_scroll()`

Toggle auto-scroll to latest bar on new data.

---

## Chart Type

### `set_chart_type(type)`

```ts
set_chart_type(type: 'candlestick' | 'ohlc' | 'line' | 'area' | 'heikin_ashi' | 'baseline' | 'footprint'): void
```

Aliases: `'candles'` = `'candlestick'`, `'bars'` = `'ohlc'`, `'ha'` = `'heikin_ashi'`, `'fp'` / `'order_flow'` = `'footprint'`.

### `set_footprint_options(json)`

Supported semantic theming keys:

- `palette`: `"blue_red"` (default) or `"green_red"`
- `gradient_style`: `"soft_glow"` (default), `"strong_glow"`, or `"no_glow"`
- `poc_color`: CSS color string or `[r, g, b, a]`
- `display_mode`, `tick_size`, `imbalance_ratio`, `show_imbalances`, `show_poc`, `show_value_area`, `value_area_pct`, `show_delta_bar`, `show_volume_text`, `show_unfinished_auction`, `show_cumulative_delta`, `font_size`, `min_cell_height`, `zoom_price_with_time`

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

## Execution Marks

First-class trade execution visualization with timestamp-based placement. Unlike series markers, execution marks are placed by timestamp (not bar index) and the engine resolves them to bars internally.

### `set_execution_marks(ids, mark_data)`

Bulk set/replace all execution marks from flat arrays.

```ts
set_execution_marks(ids: string[], mark_data: Float64Array | number[]): void
```

Each mark uses a 5-value stride:

- `timestamp_ms`
- `price`
- `quantity`
- `side_idx` (`0 = buy`, `1 = sell`)
- `role_idx` (`0 = entry`, `1 = scale_in`, `2 = scale_out`, `3 = exit`)

### `set_execution_marks_json(json)`

Bulk set/replace all execution marks from JSON.

```ts
set_execution_marks_json(json: string): void
```

**JSON format:**

```json
[
  {
    "id": "trade-001",
    "timestamp_ms": 1710000000000,
    "price": 42150.50,
    "quantity": 0.5,
    "side": "buy",
    "role": "entry",
    "order_type": "market",
    "realized_pnl": null,
    "label": "Long Entry",
    "color": [0, 255, 128, 255],
    "group_id": "position-001"
  },
  {
    "id": "trade-002",
    "timestamp_ms": 1710003600000,
    "price": 42500.00,
    "quantity": 0.25,
    "side": "sell",
    "role": "scale_out",
    "realized_pnl": 174.75
  }
]
```

**Required fields:**
- `id` — Unique string identifier
- `timestamp_ms` — Unix timestamp in milliseconds
- `price` — Execution price
- `quantity` — Trade quantity
- `side` — `"buy"` or `"sell"`
- `role` — `"entry"`, `"scale_in"`, `"scale_out"`, or `"exit"`

**Optional fields:**
- `order_type` — `"market"`, `"limit"`, `"stop"`, `"stop_limit"`
- `realized_pnl` — Realized P&L for closing trades
- `label` — Custom tooltip label
- `color` — `[r, g, b, a]` array (0-255 each)
- `group_id` — Group identifier for related trades (e.g., same position)

### `clear_execution_marks()`

Remove all execution marks from the chart.

### `set_execution_mark_text_visible(visible)`

Show or hide execution mark text labels without changing the marks themselves.

```ts
set_execution_mark_text_visible(visible: boolean): void
get_execution_mark_text_visible(): boolean
```

### `set_execution_mark_connection_line_visible(visible)`

Show or hide the selected execution connection line. When disabled, selected marks
can still render their execution chevrons without the connecting dashed line.

```ts
set_execution_mark_connection_line_visible(visible: boolean): void
get_execution_mark_connection_line_visible(): boolean
```

### `remove_execution_mark(id)`

Remove a single execution mark by ID.

```ts
remove_execution_mark(id: string): boolean
```

Returns `true` if the mark was found and removed.

### `get_execution_marks_json()`

Get all execution marks as JSON string.

```ts
get_execution_marks_json(): string
```

### Visual Rendering

Execution marks are rendered with visual distinction:

- **Buy vs Sell**: Different arrow directions and default colors
- **Hover**: Shows the exact execution-price chevron
- **Selection**: Shows execution chevrons and, optionally, the connection line
- **Text labels**: Can be toggled on or off dynamically
- **Connection line**: Can be toggled on or off dynamically

Custom colors override default styling when provided.

### Example

```ts
// Set execution marks for a trade
chart.set_execution_marks(JSON.stringify([
  {
    id: 'entry-1',
    timestamp_ms: 1710000000000,
    price: 42150.50,
    quantity: 1.0,
    side: 'buy',
    role: 'entry',
    group_id: 'position-1'
  },
  {
    id: 'scale-out-1',
    timestamp_ms: 1710007200000,
    price: 42400.00,
    quantity: 0.5,
    side: 'sell',
    role: 'scale_out',
    realized_pnl: 124.75,
    group_id: 'position-1'
  },
  {
    id: 'exit-1',
    timestamp_ms: 1710010800000,
    price: 42300.00,
    quantity: 0.5,
    side: 'sell',
    role: 'exit',
    realized_pnl: 74.75,
    group_id: 'position-1'
  }
]));

// Listen for execution mark interactions
chart.on('executionMarkClick', (event) => {
  console.log('Clicked:', event.id, event.side, event.role);
});

chart.on('executionMarkHover', (event) => {
  if (event.id) {
    showTooltip(event);
  } else {
    hideTooltip();
  }
});

// Clear all marks
chart.clear_execution_marks();
```

---

## Coordinate Helpers

### `project_point(timestamp_ms, price)`

Project a timestamp/price coordinate to canvas pixel coordinates.

```ts
project_point(timestamp_ms: number, price: number): { x: number; y: number; visible: boolean }
```

Returns:
- `x`, `y` — Canvas pixel coordinates
- `visible` — Whether the point is within the visible viewport

### `timestamp_to_bar_index(timestamp_ms)`

Convert a timestamp to a bar index.

```ts
timestamp_to_bar_index(timestamp_ms: number): number | null
```

Returns `null` if no bar matches the timestamp.

### `bar_index_to_timestamp(bar_index)`

Convert a bar index to a timestamp.

```ts
bar_index_to_timestamp(bar_index: number): number | null
```

Returns `null` if the bar index is out of range.

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
| `executionMarkClick` | `{ id, timestampMs, price, side, role, quantity, groupId }` |
| `executionMarkHover` | `{ id, timestampMs, price, side, role, quantity, groupId }` (all fields nullable when unhovered) |
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
