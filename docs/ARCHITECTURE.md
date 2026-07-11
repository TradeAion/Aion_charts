# Aion Charts — Engine Architecture Plan (Rust + WebGPU + WASM)

Goal: a production trading-chart engine with the **visual fidelity and API ergonomics of
lightweight-charts** and the **long-term extensibility of a TradingView-class platform**
(indicators, drawings, multi-pane, plugins), powered by Rust compiled to WASM rendering through
WebGPU (with a WebGL2 fallback path).

Companion document: [RENDERING_SPEC.md](RENDERING_SPEC.md) — the exact pixel math we replicate.

---

## 1. What we learned from lightweight-charts (and keep)

Their architecture is a textbook layered MVC and it maps cleanly onto Rust:

| LWC layer | Responsibility | Aion equivalent |
|---|---|---|
| `api/` | Public façade, options merging, data validation | TS package `@aion/charts` + `aion-api` (Rust) |
| `model/` | ChartModel, Pane, Series, TimeScale, PriceScale, Crosshair, DataLayer | `aion-core` crate (pure, platform-free) |
| `views/` | Per-source pane/axis views: convert model → renderer data, cache & invalidate | `aion-core::views` |
| `renderers/` | Stateless draw routines on an abstract 2D target | `aion-render` draw-list builders |
| `gui/` | Canvas/DOM management, event handling, RAF loop, layout | `aion-wasm` (host shell) + `aion-render-wgpu` |

Key ideas we keep verbatim:

1. **Integer time-point indices** as the horizontal domain; timestamps only matter at the data
   boundary and for label formatting. The union of all series' times forms the shared point list.
2. **Media(CSS)-space model, bitmap-space rasterization** — every primitive is positioned in CSS px
   floats and converted with `round(v * pixelRatio)` at encode time. This is *the* reason LWC looks
   crisp; we reproduce it exactly (see spec §2–§7 parity/rounding rules).
3. **Invalidation mask** with levels None/Cursor/Light/Full + explicit time-scale invalidation
   queue, merged and flushed once per RAF.
4. **Two-layer rendering per pane**: static layer (background/grid/series) vs dynamic layer
   (crosshair/top primitives) so mouse-move never re-renders series.
5. **View caching**: each series has pane views that rebuild items only on data/options change and
   re-convert coordinates only on scale change.
6. **IHorzScaleBehavior abstraction** — the time axis is pluggable (time / price / yield-curve
   behaviors). We keep this trait so non-time charts (options chains, curves) come free.
7. **Plugin model**: series primitives, pane primitives, custom series with z-ordered
   (bottom/normal/top) views on pane, price axis and time axis + hit-testing + autoscale hooks.
8. Axis sizing negotiation (optimal width/height, grow-fast shrink-lazy, even-px snapping),
   label overlap resolution, tick generation algorithms, magnet crosshair, kinetic scroll.

What we deliberately change:

- Canvas2D → **WebGPU draw lists** (instanced quads, polylines, glyph atlas). Rendering becomes
  retained *per frame*: views emit primitive lists, a backend encodes them.
- Single WASM heap owns all data (no JS objects per bar). Data ingestion via typed arrays.
- Multi-chart ready: one GPU device/queue shared across chart instances; one canvas per chart
  (or one canvas per app with viewports — see §6.6).

---

## 2. Workspace layout

Naming convention (project-wide): **snake_case everywhere** — crate names, module/file names, and
the public TypeScript API (`create_chart`, `add_series`, `set_data`), diverging deliberately from
lightweight-charts' camelCase.

```
aion_charts/
├─ crates/
│  ├─ aion_core/          # platform-free chart model (no wasm, no gpu deps)
│  │  ├─ src/
│  │  │  ├─ model/        # chart, pane, series, time_scale, price_scale, crosshair, magnet,
│  │  │  │                # data_layer, plot_list, range, invalidate_mask, kinetic
│  │  │  ├─ views/        # series pane views, axis views, grid, price_line, crosshair views
│  │  │  ├─ scale/        # horz_scale_behavior trait + time/price impls, tick_marks,
│  │  │  │                # price_tick_span, weight generator
│  │  │  ├─ format/       # price/percent/volume/date/time formatters, text width abstraction
│  │  │  └─ options/      # all option structs + defaults (mirror LWC defaults)
│  ├─ aion_render/        # DrawList IR: primitives, layers, text runs; no gpu deps
│  ├─ aion_render_wgpu/   # wgpu backend: pipelines, glyph atlas, batching, surfaces
│  ├─ aion_wasm/          # wasm-bindgen shell: DOM events, ResizeObserver, RAF, clipboard,
│  │                      # canvas surface creation, JS callback plumbing
│  └─ aion_native/        # (later) winit host for desktop screenshots & goldens tests
├─ packages/
│  └─ charts/             # TypeScript API package (thin, mirrors LWC API semantics, snake_case)
├─ docs/
└─ tests/
   ├─ goldens/            # image-diff tests vs lightweight-charts reference renders
   └─ unit/
```

Dependency rules: `aion_core` depends on nothing platform-specific (usable for server-side
rendering & native apps later). `aion_render` knows primitives, not GPUs. Only `aion_render_wgpu`
touches wgpu. Only `aion_wasm` touches the DOM.

---

## 3. Core model (aion-core)

Direct port of the LWC model with Rust idioms:

- `ChartModel { time_scale, panes: Vec<Pane>, crosshair, magnet, options, serieses: SlotMap<SeriesId, Series> }`
- `Pane { data_sources, left/right PriceScale + overlay scales, stretch_factor, grid }`
- `Series` is an enum-dispatched struct over `SeriesKind` (Candlestick, Bar, Line, Area, Baseline,
  Histogram, Custom) sharing one plot-row storage.
- **Storage: SoA.** `PlotList` = `{ indices: Vec<u32>, time_keys: Vec<i64>, values: [Vec<f64>; 4] }`
  (open/high/low/close; single-value series alias all four like LWC does). Binary search on indices;
  chunked min-max cache (chunk = 30) for autoscale, as in the spec §14.
- `DataLayer` maintains the merged time-point list and per-series rows with the same
  `firstChangedPointIndex` diffing + incremental single-bar update path (spec §14) — this is what
  makes streaming updates O(1)-ish and must not be simplified away.
- `InvalidateMask` — same levels & time-scale invalidation queue; mutations produce masks; the host
  merges and schedules one RAF.
- `HorzScaleBehavior` trait (generic over the horizontal item):

```rust
trait HorzScaleBehavior {
    type Item;                       // e.g. UtcTimestamp | BusinessDay
    fn key(&self, item: &Self::Item) -> i64;
    fn fill_weights(&self, points: &mut [TimeScalePoint], start: usize);
    fn format_tick(&self, mark: &TickMark, loc: &Localization) -> String;
    fn format_crosshair(&self, point: &TimeScalePoint) -> String;
    fn max_tick_weight(&self, marks: &[TimeMark]) -> u8;
}
```

- Formatters ported 1:1 (price formatter with U+2212 minus, percent, volume, date/time with the
  same format tokens). Locale month/day names via a small embedded table + optional JS `Intl`
  callback override (LWC allows `localization.timeFormatter`/`priceFormatter` — we expose the same
  as JS callbacks; they're called only for visible labels, so the JS boundary cost is negligible).
- Timezone policy = LWC policy: engine is UTC/naive; support timezones by (a) user-side timestamp
  shifting, (b) `timeFormatter`/`tickMarkFormatter` overrides, (c) optional built-in tz shifting
  behind a `chrono-tz` feature for the Rust-native use case.

Numerics note: LWC math is f64 everywhere (JS). We use f64 in the model and convert to f32 only
inside draw-list encoding, after media→bitmap rounding — this keeps price math exact for
crypto-scale values (1e-8 ticks) and avoids f32 jitter when zoomed into large timestamps.

---

## 4. View layer & draw-list IR (aion-render)

Views keep LWC's caching discipline (`_dataInvalidated / _optionsInvalidated / _invalidated`,
`visibleTimedValues` slicing) but emit a **DrawList** instead of calling Canvas2D:

```rust
enum Prim {
    // integer bitmap-space, matches fillRect semantics (spec §2-4, §7)
    Rect        { x: i32, y: i32, w: i32, h: i32, color: Color },
    RectFrame   { rect: IRect, border: i32, color: Color },        // fillRectInnerBorder
    // float bitmap-space, anti-aliased (spec §5)
    Polyline    { points: Range<u32>, width: f32, color_spans: .., style: LineStyle, line_type: LineType, cap: Cap, join: Join },
    AreaFill    { path: Range<u32>, base_y: f32, gradient: Gradient },
    HLine/VLine { coord: i32, from: i32, to: i32, width: i32, style: LineStyle, color: Color },
    RoundRect   { rect: FRect, radii: [f32;4], fill: Color, border: Option<(f32, Color)> },
    Circle      { center, radius, fill, stroke },                   // crosshair marker, point markers
    Text        { run: TextRunId, x: f32, y: f32, color: Color, align, baseline },
    ClipPush(IRect) / ClipPop,
}
struct Layer { prims: Vec<Prim>, points: Vec<[f32;2]> }   // main vs top per pane/axis
struct DrawList { panes: Vec<PaneLayers>, axes: ..., background: ... }
```

Design points:

- Candles/wicks/histograms are **Rect runs** — thousands of same-color rects collapse into one
  instanced draw. Color batching mirrors LWC's `fillStyle` caching by sorting runs per color
  *while preserving z within a series pass* (wicks → borders → bodies).
- Dashed lines: implemented in the polyline shader via distance-along-line (we carry the LWC dash
  patterns and the accumulated dash-offset behavior across color changes).
- The two-canvas trick becomes **two layers per pane rendered into the same surface**, but with the
  static layer cached in an offscreen texture: on `Cursor` invalidation we only re-encode the top
  layer and composite `static_texture + top layer` — same perf characteristics as LWC, one canvas.
- Gradients: vertical linear only (background, area) — trivially in-shader.

## 5. WebGPU backend (aion-render-wgpu)

Pipelines (all `Rgba8Unorm`/`Bgra8Unorm`, premultiplied alpha, single render pass per frame per chart):

1. **SolidQuad** — instanced integer rects (candles, wicks, histograms, grid ticks, borders,
   crosshair lines, axis label boxes without radius). Vertex expansion in shader from
   `{i32 x,y,w,h, u32 color}` instance buffer. No MSAA needed — edges are pixel-aligned by
   construction, which *exactly* matches Canvas2D fillRect output.
2. **Polyline** — CPU-tessellated triangle strips (round joins per LWC `lineJoin:'round'`,
   butt caps) with edge AA via signed-distance feathering (1px smoothstep), dash pattern via
   per-vertex arc-length. CPU tessellation (lyon or hand-rolled) is fine: visible points ≤ ~4k
   after conflation; a line strip re-tessellates only on Light invalidation.
3. **RoundRect SDF** — axis labels, markers (per-corner radius, border, spec §10 geometry).
4. **AreaFill** — triangle fan between polyline and base with vertical gradient.
5. **Glyph** — instanced textured quads from the glyph atlas.
6. **Blit/Composite** — static-layer texture → surface, then dynamic prims on top.

Text stack:

- `cosmic-text` (shaping + fallback) rasterizing into an `etagere`-packed R8 atlas, **rasterized at
  physical pixel size per DPR** and positioned at integer bitmap coords with the same
  `yMidCorrection` trick (measure ascent/descent, center on the label box) — this is what makes
  axis text look like Canvas2D `textBaseline:'middle'`.
- Text measurement (`TextWidthCache`, 200-entry LRU keyed by string with digit-normalization like
  LWC) lives in `aion-core` behind a `MeasureText` trait implemented by the glyph engine.
- Font default: same stack as LWC (`-apple-system, ..., sans-serif`) — we load the platform
  sans via `local()` queries in JS and pass the bytes in; ship a bundled fallback (e.g. Inter)
  for deterministic tests.

Fallback: a WebGL2 backend later via wgpu's GL backend (same code) — WebGPU coverage in 2026 is
good (Chrome/Edge/Firefox stable, Safari 26+) but a fallback matters for a production product.
Worst-case fallback is a Canvas2D executor for the same DrawList IR (cheap to write, guaranteed
correct, slow — nice for SSR/screenshots too).

Frame loop & surfaces:

- One `<canvas>` per chart, `configure()`d at bitmap size = suggested even CSS size × DPR
  (spec §12 sizing rules; ResizeObserver device-pixel-content-box when available).
- RAF callback: drain merged InvalidateMask → run model updates (autoscale, time-scale
  invalidations, animations) → rebuild dirty views → encode draw list → submit. Skip encode
  entirely at level None; only top layers at Cursor.

---

## 6. Host shell & public API

### 6.1 JS/TS package (`@aion/charts`)

Mirror the LWC v5 API surface so users (and our future platform code) get a familiar contract:

```ts
const chart = create_chart(container, options?);
const series = chart.add_series(candlestick_series, options?, pane_index?);
series.set_data(bars); series.update(bar, historical_update?);
series.create_price_line/price_scale()/apply_options()/price_to_coordinate()...
chart.time_scale(): fit_content, scroll_to_position, set_visible_range,
  (get/set)_visible_logical_range, subscribe_visible_time_range_change,
  time_to_coordinate/coordinate_to_time, apply_options...
chart.price_scale(id, pane_index?), chart.panes(), pane.move_to/set_height/set_stretch_factor...
chart.subscribe_crosshair_move/subscribe_click/subscribe_dbl_click, chart.take_screenshot(),
chart.apply_options(), chart.remove()
```

(API is a semantic mirror of lightweight-charts v5 but named in snake_case per project convention.)

- Options objects are structurally identical to LWC's (defaults from spec §15) → drop-in
  migration story and we can reuse their docs/mental model.
- Data crossing the boundary: `setData` accepts JS arrays but immediately packs to typed arrays
  (`Float64Array` time/o/h/l/c + color side-tables) in the TS layer; the WASM side reads via one
  `memcpy`. `update()` is a flat struct call. Zero per-bar JS objects retained.
- Events out: one shared `Float64Array` scratch + a small enum tag; JS wrappers materialize the
  familiar `MouseEventParams {time, logical, point, seriesData, hoveredSeries, hoveredObjectId}`
  lazily (getter-based) so unsubscribed/unused fields cost nothing.

### 6.2 Input handling

Port LWC's `MouseEventHandler` gesture recognizer to TS in the shell (tap/double-tap/long-tap,
pressed-move, pinch with distance ratio, page-scroll heuristics, `preventDefault` rules,
double-click 500ms window) and forward normalized gestures to Rust:
`gesture(kind, x, y, extra)` — the model code (scroll/scale/kinetic/tracking-mode) lives in Rust.
Wheel handling per spec §1.1 (delta modes, Windows-Chrome DPR correction).

### 6.3 Plugin system (critical for the TradingView ambition)

Three tiers, mirroring LWC's proven design (spec §16, LWC `plugins/`):

1. **Rust plugins (first-party, fast path)** — traits compiled in:
   `SeriesPrimitive`, `PanePrimitive`, `CustomSeries` (own the value→plot mapping + draw),
   each contributing z-ordered draw-list fragments + hit-test + autoscale + axis views. All
   built-ins (series markers, price lines, up/down markers, watermarks) are implemented as these,
   proving the API like LWC does.
2. **JS plugins (compat path)** — a `CanvasRenderingContext2D`-like recording proxy: JS primitive's
   `draw(target)` receives a recorder implementing the ~20 ctx methods LWC plugins actually use
   (fillRect, moveTo/lineTo/stroke, arc, fillText, roundRect, save/restore, transforms); commands
   are decoded into DrawList prims. This lets the existing LWC plugin ecosystem (and user
   drawings written against it) run mostly unmodified. Document the unsupported exotica
   (e.g. `createPattern`, arbitrary clip paths) and add on demand.
3. **Drawings/tools layer (future)** — built on tier 1 with serialized state, hit-testing from the
   pane hit-test machinery, magnet integration.

### 6.4 Indicator/compute layer (future-proofing)

Keep the engine data-in/draw-out. Indicators are producers that own output series — same model as
LWC's `indicator-examples`. Rust-side compute API later (`aion-indicators` crate) with the same
series primitives; nothing in the core needs to know.

### 6.5 Multi-chart & layouts

`createChart` instances share one wgpu `Device/Queue/Atlas` via a module-level singleton (big
memory win for 8-chart layouts). Time-scale sync = the LWC pattern
(`subscribeVisibleLogicalRangeChange` → `setVisibleLogicalRange`) exposed natively:
`synchronizeTimeScales(chartA, chartB)` helper in TS.

### 6.6 Server/native rendering

Because `aion-core` + `aion-render` are platform-free, `aion-native` (wgpu on Vulkan/Metal +
readback) gives golden-image tests, server-side chart PNGs, and a path to desktop apps.

---

## 7. Performance plan

Targets (mid-range laptop, 4k-wide chart):

- 60 fps pan/zoom with 10 series × 50k visible-range bars; crosshair move < 0.5 ms CPU.
- 1M-bar series load < 300 ms; streaming update < 0.1 ms.

Mechanisms:

- Visible-range slicing before any per-bar work (LWC does this; keep it).
- SoA + no per-frame allocation: draw lists and tessellation buffers are pooled and reused;
  instance buffers are persistently mapped (write-through ring buffer).
- Coordinate conversion is a tight SIMD-friendly loop (`indexesToCoordinates`,
  `barPricesToCoordinates` are branch-free affine transforms — spec §1).
- Static/top layer split (§4) makes crosshair-only frames ~free.
- Data conflation (LWC v5 has it: power-of-2 bucket merge when barSpacing < 1/DPR px, OHLC-correct
  merging) — port `data-conflater.ts` in phase 4; it's the difference between "fine" and
  "instant" for 1M+ points zoomed out.
- No GC pressure: events and data cross the boundary through preallocated typed arrays.

---

## 8. Fidelity & testing strategy

1. **Port the math with its tests.** LWC has unit tests for tick span, formatters, data layer,
   plot list. Port test vectors into Rust unit tests (`tests/unit`).
2. **Golden-image diffing.** Render fixed scenarios (candles at bar spacings 0.5–50, DPR 1/1.25/2/3,
   both themes, axis label edge cases, dashed styles, histograms with tiny spacing) in
   lightweight-charts via headless Chromium → PNG; render the same scene in `aion-native` → PNG;
   assert per-pixel diff within tolerance (rects must be *exact*; AA lines/text get a small
   perceptual tolerance). These live in `tests/goldens` and run in CI.
3. **Interaction parity tests.** Scripted gesture sequences (wheel-zoom at x, drag, pinch, axis
   drag/double-click) asserting `(barSpacing, rightOffset, priceRange)` match LWC values to 1e-9 —
   the formulas in spec §1 are deterministic, so this is exact.
4. Property tests on scales (roundtrip `price↔coordinate`, `index↔coordinate`, autoscale
   invariants, log-formula switching).

---

## 9. Phased roadmap

**Phase 0 — Skeleton. ✅ DONE (2026-07-11).** Workspace (`aion_core`, `aion_render`,
`aion_render_wgpu` with wgpu 25, `aion_wasm`), solid-quad pipeline (instanced integer rects,
premultiplied alpha, no MSAA), quad executor (Rect/RectFrame/HLine/VLine incl. CPU dash
expansion), wasm-bindgen chart shell with canvas surface, zoom/pan/crosshair/fit gestures,
resize with fractional DPR. Demo at `examples/web_demo` (serve via `.claude/launch.json`,
build with `wasm-pack build crates/aion_wasm --target web --out-dir ../../examples/web_demo/pkg`).
Verified in-browser: only exact palette colors on canvas (zero AA bleed), zoom compounds at
exactly 1.1^n, dashed crosshair coverage exactly 50%.

**Phase 1 — The chart core (3–4 wks). PARTIALLY DONE.** `aion_core` port: time scale ✅,
price scale (all 4 modes + log formula) ✅, tick span calculator ✅, invalidate mask ✅,
formatters (price/percent/volume) ✅, candlestick geometry ✅ (66 unit tests).
Remaining: data layer + plot list (merged time points, whitespace, chunked min/max cache),
bar + histogram renderers, options structs, RAF/invalidation plumbing in the shell,
time tick marks (weights) for vertical grid. Then first goldens.
*Exit: a candlestick chart with grid, correct at every bar spacing and DPR.*

**Phase 2 — Axes & text (2–3 wks). PARTIALLY DONE.**

Text architecture decision (2026-07): axes render on a **stacked Canvas2D overlay**, not the
GPU — mirroring LWC's per-cell canvas layout. The pane is one WebGPU canvas; a second
full-size Canvas2D canvas sits exactly on top, transparent except over the axis strips, and
Rust draws all axis chrome/labels to it via web-sys `fillText`/`fillRect`. This gives native,
premium axis text (the product goal) while all layout/format logic stays in Rust. The GPU
label atlas + textured-quad pipeline are kept, constructed-but-idle, reserved for future
*in-pane* text (legend, watermark, series markers) — a canvas is `webgpu` XOR `2d`, so
anything inside the GPU pane still needs the atlas. Tradeoff accepted: the native/`takeScreenshot`/
golden path will still need the atlas text path, and the time-axis overlay can desync from the
GPU pane by a frame under heavy jank (price axis is immune — it doesn't move on horizontal pan).

Done: hybrid pane(GPU)/axes(2D) rendering, price axis (optimal-width formula via real browser
metrics, border, tick labels), time axis (28px optimalHeight, weight-based labels, en-US tick
formatter), crosshair axis labels (dark boxes + white text), all native 2D. Remaining: label
overlap resolution, bold high-weight time labels, `Intl` locale hook, edge tick marks,
rounded crosshair-label corners, axis drag-scale gestures, goldens.
*Exit: goldens of full chart with both axes match LWC.*

**Phase 3 — Interaction (2–3 wks).** Gesture recognizer, wheel/pinch zoom, pan, axis drag scale,
double-click reset, kinetic scroll, crosshair + magnet + tracking mode (touch), axis crosshair
labels, hit testing, click/crosshair subscriptions. Interaction parity tests.
*Exit: feels identical to LWC side-by-side.*

**Phase 4 — Series completeness & streaming (2–3 wks). PARTIALLY DONE.** Done: line + area
series (CPU-tessellated polyline stroke with round joins, gradient area fill, 4x MSAA for edge
AA — pixel-aligned rects/text stay bit-identical), triangle pipeline, MSAA frame target,
`update()` streaming path, new-bar shift, **magnet crosshair** (Normal/Magnet/MagnetOHLC/Hidden,
default Magnet like LWC — snaps horizontal line to close/OHLC), **crosshair marker** (white halo
+ series-color disc on line/area), **last-value price line + colored axis label** (dashed line
to last close, contrast text). Remaining: baseline series, WithSteps/Curved line types, point
markers, last-price animation, whitespace handling, custom price lines, data conflation port.

**Phase 5 — Multi-pane & platform features (2–3 wks).** Panes + separators + resize, overlay
price scales, `moveSeriesToPane`, pane primitives, watermark, screenshot, autoSize, multi-chart
device sharing, time-scale sync helper.

**Phase 6 — Plugin surface & hardening (ongoing).** JS plugin recorder, series/pane primitive JS
API, custom series API, WebGL2/Canvas2D fallback executor, perf pass (1M-bar benchmarks),
docs site with LWC-style examples, drawings/tools groundwork.

---

## 10. Key risks & mitigations

| Risk | Mitigation |
|---|---|
| Text crispness vs Canvas2D | Per-DPR rasterization, integer placement, yMidCorrection; golden tests at DPR 1/1.25/2/3; fallback: rasterize labels via hidden 2D canvas into the atlas (pixel-identical by construction) |
| WebGPU availability / context loss | wgpu GL backend + Canvas2D DrawList executor; device-lost → recreate & full invalidate |
| JS↔WASM chattiness | Typed-array batching, lazy event params, callbacks only for visible labels |
| f32 precision at deep zoom | f64 model math; translate-then-scale in encode so vertex coords stay small |
| Scope creep toward TradingView | Phases 1–4 ship a LWC-equivalent product; platform features are additive because the plugin/z-order/pane architecture is in from day one |
| LWC behavioral corner cases (axis width shake, label overlap, whitespace) | We ported the *reasons* (grow-fast/shrink-lazy, parity snapping, prevEdge) into the spec; goldens catch regressions |

---

## 11. Immediate next steps

1. Scaffold the cargo workspace + TS package + wasm build (wasm-pack or trunk; `wasm32-unknown-unknown`,
   `wasm-bindgen`, `wgpu` with `webgpu` backend).
2. Implement `aion-core::scale::time_scale` + `price_scale` with ported unit tests (pure math first —
   no rendering deps, fastest way to lock correctness).
3. SolidQuad pipeline + candlestick renderer → first golden comparison against LWC.
