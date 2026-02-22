# RayCore — LWC Parity Implementation Plan

## Phase 1: Crosshair Labels + Rendering Correctness

### 1.1 Price Axis Crosshair Label Sizing + Clamping
- [x] Clamp label width to not exceed price axis canvas width
- [x] Add vertical clamping: push label inward when crosshair near top/bottom edge
- Files: `src/core/renderer/price_axis.rs`

### 1.2 Time Axis Crosshair Label Sizing
- [x] Ensure label height fits within time axis height
- [x] Account for `labelBottomOffset` in height calculation
- Files: `src/core/renderer/time_axis.rs`

### 1.3 Grid Lines in WebGPU Mode
- [x] `draw_grid()` in wgpu_backend.rs now includes grid lines (not just background)
- [x] Uses same `generate_grid_rects` as Canvas2D path
- Files: `src/core/renderer/wgpu_backend.rs`, `geometry_generator.rs`

### 1.4 Line Series Dash Patterns
- [x] Canvas2D overlay: add `strokePath` mode using `setLineDash()` with LWC dash table
- [x] LWC patterns: Solid=[], Dotted=[w,w], Dashed=[2w,2w], LargeDashed=[6w,6w], SparseDotted=[w,4w]
- Files: `src/core/renderer/line_generator.rs`, `overlay.rs`

---

## Phase 2: Text Rendering Infrastructure

### 2.1 Text Width Cache
- [x] Implement bounded FIFO cache (max_size=50) with digit normalization
- [x] Replace all `measure_text()` calls with cached measurements
- Files: New `src/core/renderer/text_cache.rs`, `price_axis.rs`, `time_axis.rs`

### 2.2 yMidCorrection
- [x] Use `actualBoundingBoxAscent/Descent` for precise vertical text centering
- [x] Apply to crosshair labels and tick labels
- Files: `text_cache.rs`, `price_axis.rs`, `time_axis.rs`

---

## Phase 3: Watermark + Legend + Last Price Line

### 3.1 Watermark Rendering
- [x] Render centered watermark text on pane (on overlay canvas, below drawings/crosshair)
- [x] Auto-zoom shrink if text wider than pane
- [x] Use existing `watermark_color` and `font_size_watermark` from ChartStyle
- [x] Added `watermark_text` field to ChartStyle + `set_watermark()` WASM API
- Files: `src/core/renderer/overlay.rs`, `traits.rs`, `theme.rs`, `wasm/src/lib.rs`

### 3.2 Legend / OHLC Values Display
- [x] Render OHLC + Volume values in top-left corner of pane
- [x] Update on crosshair hover (show values at hovered bar)
- [x] Color-coded bullish/bearish
- Files: `overlay.rs`

### 3.3 Series Last Price Line
- [x] Horizontal dashed line at series' last value, full pane width
- [x] Small label on price axis showing current price
- Files: `overlay.rs`, `src/core/series/mod.rs`

### 3.4 Crosshair Marker Circle on Series
- [x] Filled circle at crosshair intersection with line/area/baseline series
- [x] Two-pass rendering: border ring then fill dot
- Files: `overlay.rs`

---

## Phase 4: Price Scale Modes

### 4.1 Logarithmic Price Scale
- [x] Add `PriceScaleMode` enum: Normal | Logarithmic | Percentage | IndexedTo100
- [x] Implement `to_log/from_log` with adaptive LogFormula
- [x] Apply log transform in `price_to_css_y` and `pixel_to_price`
- Files: `src/core/viewport.rs`, `tick_marks.rs`, `price_axis.rs`

### 4.2 Percentage Price Scale
- [x] `toPercent`: `100 * (value - firstValue) / firstValue`
- [x] Price labels show `+2.50%` format
- Files: `viewport.rs`, `formatters.rs`

### 4.3 IndexedTo100 Price Scale
- [x] `toIndexedTo100`: `100 * (value - firstValue) / firstValue + 100`
- Files: `viewport.rs`, `formatters.rs`

### 4.4 WASM API for Scale Mode
- [x] `set_price_scale_mode("normal"|"logarithmic"|"percentage"|"indexed_to_100")`
- Files: `wasm/src/lib.rs`

---

## Phase 5: Price Lines + Series Markers

### 5.1 Custom Price Lines
- [x] `create_price_line(price, color, style, width, label)` API
- [x] Hit-testable (7px threshold), draggable vertically
- [x] All LineStyle dash patterns
- Files: New `src/core/price_line.rs`, `overlay.rs`

### 5.2 Series Markers
- [x] Shapes: arrow up/down, circle, square, text
- [x] Positioned at bar index + above/below/at price
- [x] Two-pass circle batch rendering
- Files: New `src/core/markers.rs`, `overlay.rs`

---

## Phase 6: Multi-Pane Support

### 6.1 Pane Model
- [x] N panes with independent price scales
- [x] Stretch-factor proportional sizing
- Files: `engine.rs`, new `src/core/pane.rs`

### 6.2 Pane Separators
- [x] Draggable separator divs between panes
- [ ] Wire separator drag events to resize panes
- Files: `wasm/src/canvas_manager.rs`

### 6.3 Indicator Panes
- [ ] RSI, MACD in separate panes below main chart
- Files: `studies/manager.rs`, `wasm/src/lib.rs`

### 6.4 Study Auto-Attach
- [ ] `create_study("rsi")` auto-creates sub-pane + series + wiring
- Files: `engine.rs`, `wasm/src/lib.rs`

---

## Phase 7: Runtime Configuration API

### 7.1 Style Mutation API
- [x] `set_background_color()`, `set_grid_color()`, etc.
- [x] `set_bullish_color()`, `set_bearish_color()`, `set_volume_colors()`
- [x] `set_crosshair_color()`, `set_crosshair_label_bg_color()`, `set_crosshair_label_text_color()`
- [x] `set_font_size()`, `set_font_family()`, `set_bar_width_ratio()`
- [x] `set_price_scale_margins(top, bottom)`
- Files: `wasm/src/lib.rs`, `traits.rs`

### 7.2 Label Overlap Prevention
- [x] Push overlapping price axis labels apart vertically
- [x] Added `LabelRect` struct and `resolve_label_overlaps()` helper
- Files: `price_axis.rs`

---

## Phase 8: Additional Drawing Tools

### 8.1 Horizontal Line
- [x] Single-anchor, spans full pane width, draggable vertically
- [x] Created `horizontal_line.rs` with HorizontalLine struct
- Files: New `src/core/drawings/horizontal_line.rs`

### 8.2 Vertical Line
- [x] Single-anchor, spans full pane height, draggable horizontally
- [x] Created `vertical_line.rs` with VerticalLine struct
- Files: New `src/core/drawings/vertical_line.rs`

### 8.3 Ray / Extended Line
- [x] 2-anchor line extending to visible area edges
- [x] Created `ray.rs` with Ray struct and line extension math
- [ ] Implement Drawing trait for all three (placeholder using TrendLine)
- Files: New `src/core/drawings/ray.rs`

---

## Phase 9: Additional Studies

### 9.1 Bollinger Bands
- [x] 3 outputs: middle SMA, upper (SMA + k*stddev), lower (SMA - k*stddev)
- Files: `src/core/studies/built_in/bollinger.rs`

### 9.2 Stochastic
- [x] 2 outputs: %K, %D
- Files: `src/core/studies/built_in/stochastic.rs`

### 9.3 ATR (Average True Range)
- [x] Wilder's smoothing, True Range calculation
- Files: `src/core/studies/built_in/atr.rs`

### 9.4 VWAP
- [x] Cumulative (Typical Price * Volume) / Cumulative(Volume)
- Files: `src/core/studies/built_in/vwap.rs`

---

## Phase 10: Polish

### 10.1 Keyboard events wired in WASM core
- [x] `on_key_down(key, ctrl, shift, alt)` API in WASM
- [x] Delete/Backspace: remove selected drawing
- [x] Escape: cancel drawing, deselect all
- [x] Arrow keys: scroll (left/right) and zoom price (up/down)
- [x] Home/End: jump to start/end of data
- [x] +/-: zoom time axis in/out
- [x] 0: reset zoom to fit all data
- Files: `wasm/src/lib.rs`, `demo/index.html`

### 10.2 Anti-aliased diagonal lines
- [x] Canvas2D already provides native anti-aliasing
- [x] Added `set_line_join("round")` to drawing geometry renderer
- [x] Line cap and join settings for smooth line endpoints
- [x] 0.5px correction for odd-width lines (LWC strokeInPixel pattern)
- Files: `src/core/renderer/overlay.rs`

### 10.3 Last price animation (pulsing dot)
- [x] Pulsing/breathing circle at right edge of last price line
- [x] Sin-wave animation for smooth pulse effect (80%-120% radius, 60%-100% opacity)
- [x] 1-second animation cycle using `js_sys::Date::now()`
- Files: `src/core/renderer/overlay.rs`, `wasm/src/lib.rs`

### 10.4 Scrollbar indicator
- [x] Visual scroll position indicator at top of time axis
- [x] Track background with semi-transparent thumb
- [x] Thumb shows visible portion of data range
- [x] Rounded thumb corners for modern look
- Files: `src/core/renderer/time_axis.rs`, `wasm/src/lib.rs`

---

## Build & Verify
```bash
cargo check --target wasm32-unknown-unknown -p raycore-wasm
wasm-pack build wasm --target web --out-dir ../pkg --release
```
