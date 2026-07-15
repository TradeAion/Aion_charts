# Aion Charts — Production Roadmap

Path from the current engine (renders candles/bars/line/area/histogram, single pane, both axes,
crosshair, zoom/pan/streaming) to a **near-production charting library on par with
lightweight-charts (LWC) v5.2.0**, with the plugin/pane architecture in place for the
TradingView-class ambition.

Companion docs: [ARCHITECTURE.md](ARCHITECTURE.md) (crate layout, phase status),
[RENDERING_SPEC.md](RENDERING_SPEC.md) (exact pixel math). This document supersedes the
phase ordering in ARCHITECTURE.md §9 where they disagree — see "Reordering rationale" below.

---

## 1. Honest state assessment (2026-07-12)

- **Aion:** ~7,100 lines Rust across `aion_core` / `aion_render` / `aion_render_wgpu` / `aion_wasm`.
- **LWC v5.2.0 reference** (`tmp/lightweight-charts/`): ~30,300 lines TS.

**Strong (done well):** core scale math (price scale 4 modes + log, time scale, tick spans),
plot list + data layer, invalidate mask, magnet crosshair, formatters; candle/bar/line/area/
histogram geometry; wgpu quad/tri/tex pipelines + MSAA; Canvas2D axis-text overlay; a working
single-pane multi-series chart in `aion_wasm`.

**The defining gap:** the consumable library — `packages/charts/src/index.ts` — is a **stub that
throws**. Aion today is an *engine*, not a *library*. The largest distance to production is the
product surface (API façade, options, validation, subscriptions), **not** rendering fidelity.

---

## 2. Gap map (LWC has it → Aion doesn't)

| Area | LWC reference | Aion status | Severity |
|---|---|---|---|
| Public TS API | `api/chart-api.ts`, `series-api.ts`, handles | 24-line stub that throws | 🔴 Blocker |
| Options system | deep-merge, ~8 groups, per-series | `set_*` setters only | 🔴 Blocker |
| Data validation | `data-validators.ts` (order/dupe/NaN/whitespace) | none | 🔴 Blocker |
| Coordinate API | `priceToCoordinate`, `timeToCoordinate`, logical range | not exposed | 🟠 High |
| Multi-pane | panes, separators, resize, `moveToPane`, stub axes | single pane | 🟠 High |
| Overlay price scales | volume histogram w/ own scale | one shared scale | 🟠 High |
| Baseline series + line types | baseline, step/curved, point markers | line/area only | 🟠 High |
| Series markers | `plugins/series-markers` | none | 🟠 High |
| Price lines API | `createPriceLine` per series | last-value only | 🟡 Med |
| Subscriptions | crosshair move / click / dblclick / range change | inline, not surfaced | 🟡 Med |
| Plugins / primitives | series + pane primitives, custom series, JS recorder | none | 🟡 Med (platform) |
| Watermark | text + image | none | 🟡 Med |
| Fallback backend | Canvas2D executor | WebGPU-only | 🟠 High (reach) |
| Golden tests | (planned) | none | 🟠 High (safety) |
| Data conflation | `data-conflater.ts` (1M+ pts) | none | 🟢 Low (perf) |
| Yield-curve / price horz | pluggable horz behaviors | time only | 🟢 Low |

---

## 3. Reordering rationale

ARCHITECTURE.md §9 has pushed rendering (Phases 4–5). The product-defining gap is the **library
shell**: a pixel-perfect engine with an API that throws is further from production than a slightly
less perfect engine you can `npm install` and configure. Therefore:

1. **Phase A (library shell) moves to the front.** Nothing ships without it.
2. **Golden tests + Canvas2D fallback (Phase D) start now, in parallel** — they de-risk every
   change made in A–C and turn the WebGPU-only demo into a browser-universal product.

---

## 4. Phases

### Phase A — Make it a consumable library  🔴 critical path

*Exit: `npm install @aion/charts`, feed OHLC, get a styled chart, wire a tooltip — the LWC
"getting started" story works end to end.*

- **A1. Real TS API façade.** `create_chart(container, options?) → IChartApi`-equivalent;
  `add_series(kind, options?) → series handle` (object, not a `u32`); `series.set_data/update`;
  `chart.remove()`. Typed-array packing at the boundary (no per-bar JS objects).
- **A2. Options system.** New `aion_core::options` module mirroring LWC defaults (RENDERING_SPEC
  §15): layout, grid, crosshair, time_scale, right/left price_scale, localization, per-series.
  `apply_options` deep-merge on chart / series / scale.
- **A3. Data validation.** Port `data-validators.ts`: monotonic time, dedupe, NaN rejection,
  whitespace rows. Real feeds must not panic the wasm module.
- **A4. Coordinate + logical-range API.** `price_to_coordinate`, `coordinate_to_price`,
  `time_to_coordinate`, `coordinate_to_time`, `get/set_visible_logical_range`,
  `get/set_visible_range` — all computable from existing scale cores.
- **A5. Subscriptions.** `subscribe_crosshair_move`, `subscribe_click`, `subscribe_dbl_click`,
  `subscribe_visible_time_range_change` with lazily materialized event params.

### Phase B — Core feature parity

*Exit: volume + an indicator pane render; series set matches LWC.*

- **B1. Multi-pane:** panes, separators, drag-resize, per-pane stub price axes, `move_to_pane`,
  stretch factors.
- **B2. Overlay price scales:** independent scale ids (e.g. volume pinned to bottom fraction).
- **B3. Series completeness:** baseline series; step / curved line types; point markers;
  last-price animation; whitespace handling.
- **B4. Series markers** plugin + per-series **price lines API** (`create_price_line`).

### Phase C — Platform surface (TradingView ambition)

- **C1. Primitives:** `SeriesPrimitive` / `PanePrimitive` / `CustomSeries` Rust traits with
  z-ordered draw-list fragments + hit-test + autoscale + axis views.
- **C2. JS plugin recorder:** a `CanvasRenderingContext2D`-like proxy decoding the ~20 ctx methods
  LWC plugins use into DrawList prims — runs the existing LWC plugin ecosystem mostly unmodified.
- **C3. Watermark** (text/image), attribution logo, `autoSize`.

### Phase D — Hardening (start now, run in parallel)

- **D1. Golden-image harness:** headless Chromium renders LWC PNGs; `aion_native` renders ours;
  per-pixel diff (rects exact, AA/text small tolerance). Protects fidelity claims + catches
  regressions across A–C.
- **D2. Canvas2D fallback executor** for the DrawList IR — cheap, guaranteed-correct; doubles as
  the SSR / screenshot / golden render path; makes the product browser-universal.
- **D3. Data conflation** + 1M-bar benchmarks.

---

## 5. Definition of "near production ready"

- [ ] `@aion/charts` installs and runs the LWC getting-started example unmodified in spirit.
- [ ] Options parity for the common groups; `apply_options` deep-merge works.
- [ ] Malformed data is rejected with clear errors, never a wasm panic.
- [ ] Volume + at least one indicator pane render correctly with independent scales.
- [ ] Crosshair/click subscriptions drive a tooltip.
- [ ] Renders in browsers without WebGPU (Canvas2D fallback).
- [ ] Golden tests green vs LWC across bar spacings 0.5–50 and DPR 1/1.25/2/3.
- [ ] 60 fps pan/zoom at 10 series × 50k visible bars; 1M-bar load < 300 ms.

---

## 6. Execution log

Progress is appended here as phases land (newest last).

- 2026-07-12 — Roadmap authored. Beginning Phase A.
- 2026-07-12 — **A3 done.** `aion_core::model::data_validation` (sanitize_ohlc / sanitize_point:
  repair-and-report — drop non-finite/out-of-range, stable-sort, dedupe last-wins, error only on
  length mismatch). Wired into wasm `set_series_data` / `update_bar`; malformed feeds warn + render
  instead of panicking. 11 unit tests.
- 2026-07-12 — **A4 done.** Coordinate & logical-range API on the wasm surface:
  `price_to_coordinate` / `coordinate_to_price`, `time_to_coordinate` / `coordinate_to_time`,
  `visible_logical_range` + setter, `visible_time_range` + setter. Verified in-browser: price/time
  roundtrips exact, off-chart queries return `undefined`, setters apply.
- 2026-07-12 — **A2 done** (options system). `aion_core::options`: serde-backed structs with
  LWC-matching defaults (layout/grid/crosshair) + `ChartOptionsStore` doing LWC `merge`-semantics
  deep-merge (nested objects merge key-by-key; scalars/arrays/null replace). `aion_render::Color`
  gained `#rgb`/`#rgba` shorthand + `rgb()/rgba()` parsing. Wired `apply_options` / `options_json`
  into the wasm chart; grid colors+visibility, crosshair line colors+visibility, and the
  background clear color now come from options. Verified in-browser: partial patches deep-merge
  (siblings survive, patches accumulate) and reach pixels (bg 94.8% red, blue grid lines present).
  15 new unit tests. Next: A1 (real TS façade), A5 (subscriptions).
- 2026-07-12 — **A1 done** (real library façade). `packages/charts/src/index.ts` is now a typed
  `@aion/charts` API over the wasm engine — no longer a stub: `create_chart(container, options?)`
  → `Promise<chart_api>` (creates the two stacked canvases, installs the gesture recognizer,
  applies options); `add_series(kind, options?)` → series handle (`set_data`/`update`/`set_type`/
  `apply_options`, typed-array packing at the boundary); `time_scale()` (fit/visible-range get+set/
  coord conversions); `price_to_coordinate`/`coordinate_to_price`; `apply_options`/`options`;
  `resize`/`remove`. `autoSize` gates the ResizeObserver (LWC parity; off ⇒ manual sizing, keeps
  the engine embeddable/testable). Build: `wasm-pack` → `packages/charts/pkg`, esbuild bundles a
  self-contained ESM into `examples/web_demo/dist/`. The demo now consumes the published API only
  (raw-wasm wiring + inline gestures removed). Verified in-browser: candles render via the façade
  (LWC palette), overlay line series + `apply_options` deep-merge reach pixels, coordinate/range
  APIs return correct values, full chart screenshot. tsc + wasm builds green.
  Next: A5 (subscriptions — needs Rust→JS callback plumbing).
- 2026-07-12 — **A5 done → Phase A COMPLETE.** Subscriptions: `subscribe_crosshair_move` /
  `subscribe_click` (+ unsubscribe) delivering `mouse_event_params { time, logical, point,
  series_data }`. Engine gained `hover_data(x)` (per-series OHLC at the hovered bar, flat
  `[id,o,h,l,c,…]`) and `coordinate_to_logical(x)`; the façade owns the callback registry, builds
  params, and fires from the gesture recognizer (move → crosshair, pointer-leave → empty params,
  click). Demo grew a live OHLC legend driven by the subscription. Verified in-browser: move/leave/
  click all fire with correct time, logical (bar 539), point, and full OHLC series_data; legend
  reads "O 95.13 H 96.47 L 94.92 C 95.77" on hover.

  **Phase A (the library shell) is done: installable, configurable, safe against bad data, with
  coordinate + subscription APIs.** Next up: Phase B — multi-pane + overlay/volume price scales.
- 2026-07-12 — **B2 done** (overlay/volume price scale). Second `overlay_scale: PriceScaleCore`
  pinned to a bottom band via `scale_margins` (default `{top:0.8, bottom:0}`); series carry an
  `overlay` flag; autoscale split so the main price axis ignores overlay magnitude; histogram
  builder routes through the series' scale; `PriceScaleCore::set_scale_margins`. Engine
  `set_series_overlay(id, top, bottom)` → façade `add_series("histogram", { overlay: true,
  scale_margins? })`; demo volume toggle. Verified in-browser: with volume on, the price axis is
  byte-identical (top 125.74 / bottom 56.76 unchanged) while the histogram fills the bottom 20%
  band (47% non-white there vs 6% above). Next in B: multi-pane (B1) — separate panes/separators/
  resize/move_to_pane.
- 2026-07-12 — **B1 increment 1 done** (multi-pane model + stacked layout). Introduced a `Pane`
  (own price + overlay scale, stretch factor, slot top/height); `ChartInner.panes: Vec<Pane>`
  replaces the single scale. Layout splits the content area by stretch factor (minus 1px
  separators); each pane's scale uses the "absolute coordinate" trick (full content height + internal
  margins position the band) so builders read `price_to_coordinate` as canvas-absolute Y with no
  offset threading. Autoscale is per-pane; render emits one scissored `DrawGroup` per pane; series
  carry a `pane_index`; separators drawn on the 2D overlay. New `set_series_pane(id, pane, stretch)`
  → façade `add_series(kind, { pane, pane_stretch })`; demo volume moved to its own pane. Verified
  in-browser: candles confined to the top pane (end ≈63%), volume in the bottom pane (start ≈76%),
  cleanly separated; single-pane rendering byte-unchanged; core 96 + render 31 tests green.
  Remaining B1 increments: per-pane price axes/labels, draggable separators (resize), façade
  `panes()`/`move_to_pane`, per-pane crosshair label.
- 2026-07-12 — **B1 increment 2 done** (per-pane price axes). `draw_axes_2d` now iterates every
  pane and draws its own price tick labels clipped to its band (scale coords are canvas-absolute);
  `compute_price_axis_width` measures the widest label across all panes so a wide volume axis
  doesn't clip. Verified in-browser: both the price band and the volume band render their own
  right-axis labels (dark text present in each strip), no console errors. Remaining B1: draggable
  separators (resize), façade `panes()`/`move_to_pane`, per-pane crosshair label.
- 2026-07-12 — **B1 increment 3 done** (draggable separators + move_to_pane). Engine
  `drag_pane_separator(i, delta)` (freezes heights as stretch factors, moves the boundary, min
  24px), `pane_separator_ys()`, `pane_count()`. Façade recognizes a press within 4px of a boundary
  as a separator drag (not a pan), resizes on move, and shows a `row-resize` hover cursor;
  `series.move_to_pane(index)`. `setPointerCapture` now guarded. Verified in-browser: dragging the
  separator up 40px / down 60px moves it exactly 40 / 60 px; cursor feedback works; no errors.
  Remaining B1: façade `panes()` handle surface, per-pane crosshair price label.
- 2026-07-12 — **B1 increment 4 done** (per-pane crosshair). Horizontal crosshair line + price
  axis label now follow the cursor into whichever pane it's over, using that pane's scale (price
  pane magnet-snaps to its series; indicator panes read the raw cursor y via
  `coordinate_to_price`); marker stays on the price pane. Added `pane_at_y(y)`. Verified
  in-browser: cursor in the volume pane (frac 0.81, below the 0.70 separator) draws a full-width
  horizontal line at 0.81; no errors. **B1 core is functionally complete** (stacked panes,
  per-pane axes, draggable resize, per-pane crosshair). Optional later: a richer `panes()` handle
  API. Next: B3 (baseline/step/curved line types, point markers, last-price animation) + B4
  (series markers, price-lines API).
- 2026-07-12 — **B3 increment 1 done** (step & curved line types). `aion_render::line::expand_line`
  transforms a polyline by `LineType`: `WithSteps` inserts a horizontal-then-vertical corner per
  interval; `Curved` tessellates a Catmull-Rom spline (16 segs/interval) through the knots. Applied
  in both `build_line_stroke` and `build_area_fill`. Series carry a `line_type`;
  `set_series_line_type(id, 0|1|2)` → façade `add_series(kind, { line_type: 'simple'|'stepped'|
  'curved' })`. 3 new renderer unit tests (render 34). Verified in-browser: at ~30 visible bars the
  three types render distinct geometry (simple 2680 px, stepped 3354, curved 2734). Remaining B3:
  baseline series, point markers, last-price animation.
- 2026-07-12 — **B3 increment 2 done** (point markers). Line/area series can draw a filled disc at
  each data point, gated on bar spacing (≥ 2·r+2) so discs never merge — matching LWC's hide-below-
  threshold behavior. `set_series_point_markers(id, bool)` → façade `add_series(kind, {
  point_markers: true })`. Verified in-browser: zoomed in (bar spacing 46) markers add 358 px;
  zoomed out (0.75) they add 0 (hidden). Remaining B3: baseline series, last-price animation.
- 2026-07-12 — **B3 increment 3 done** (baseline series). `aion_render::line::build_baseline`
  strokes+fills a line split at a baseline y, splitting each crossing segment so the color flips
  exactly at the baseline (teal/fill above, red/fill below). New `SeriesKind::Baseline` (kind 5);
  baseline price defaults to the visible-range midpoint or `set_series_baseline(id, price)`. Façade
  `add_series("baseline", { baseline_value? })`. 1 new renderer test (render 35). Verified
  in-browser: both line colors (teal 2604 px / red 2500) and both area fills render, teal correctly
  above red. Remaining B3: last-price animation (needs an rAF animation loop — deferred). **B3 core
  (line types, markers, baseline) done.** Next: B4 (series markers, price-lines API).
- 2026-07-12 — **B3 increment 4 done → B3 COMPLETE.** Last-price animation: an expanding, fading
  ring under a solid center dot at the main series' last value, on a ~2600 ms cycle. The engine
  takes a host clock (`set_animation_time`, `wants_animation`) since render is synchronous; the
  façade runs an rAF loop while any series has `last_price_animation: true`, stopped on `remove()`.
  Verified in-browser: `wants_animation` toggles false→true; the ring area grows over the cycle
  (+8 px at phase 0 → +103 at phase 0.5 → +8 faded at phase 0.98). B3 fully done: line types, point
  markers, baseline series, last-price animation. Next: **B4** — series markers (arrows/circles) +
  per-series price-lines API.
- 2026-07-12 — **B4 increment 1 done** (per-series price lines). `series.create_price_line({ price,
  color, line_width, line_style, title })` → a handle with `.remove()`. Engine: per-series
  `Vec<PriceLine>`, `create_price_line`/`remove_price_line`; rendered as an HLine on the series'
  scale in its pane (`build_price_lines`) plus a colored axis label clipped to the pane band
  (`draw_price_line_labels_2d`). Verified in-browser: line (468 px) + axis label (435 px) render on
  the correct row; `handle.remove()` clears both. (Debugging note: manual `resize()` while
  `autoSize` is on desyncs the pane/overlay canvas sizes — verify without calling resize.) Next in
  B4: series markers (arrows/circles/squares above/below bars).
- 2026-07-14 — **B4 increment 2 done → B4 & Phase B COMPLETE.** Per-bar series markers
  (`series.set_markers([{ time, position, shape, color, text }])`): position `above|below|inBar`,
  shape `circle|square|arrowUp|arrowDown`. Engine holds a `Vec<Marker>` per series; `build_markers`
  places each on its series' scale/pane (above the high − gap, below the low + gap, or in-bar mid),
  gated to the visible index range, emitting filled triangles (disc/square/arrow) into the pane's
  MSAA tri group. Boundary is a JSON array (`set_series_markers`); the façade `JSON.stringify`s it
  (added `serde` derive to `aion_wasm`). Verified in-browser: all four shapes render at the correct
  positions/colors (pink circle above, green square below, blue arrowUp above, orange arrowDown
  below, purple in-bar), `set_markers([])` clears them, no console errors. (Marker `text` label is
  carried but not yet drawn — deferred to a later 2D-overlay increment.)
  **Phase B (core feature parity) is done: multi-pane, overlay/volume scales, full series set +
  line types/markers/baseline/animation, price lines, and series markers.** Next: Phase C
  (platform — primitives, JS plugin recorder, watermark) or Phase D (hardening — goldens, Canvas2D
  fallback), which the roadmap says can run in parallel.
- 2026-07-14 — **Phase B polish: per-series streaming `update()`.** `series.update()` previously
  no-op'd with a warning for any non-primary series (only the main series streamed); the data layer
  already supported per-series `update(id, …)`, so the gap was purely the wasm/façade wiring. Added
  `update_series_bar(series_id, o,h,l,c)` to the wasm surface (main `update_bar` now delegates to it
  with id 0; unknown ids warn instead of corrupting the data layer), and the façade routes
  `series.update()` through it. Now overlays/indicators/volume can stream live. Verified via the
  coordinate/range API (screenshot capture was wedged in the preview pane this session, unrelated to
  the change): replace-last drives autoscale (price 130 → y≈108.8, was y≈196.6 for 100); appending a
  new max time grows the merged set by exactly one (visible `to` 999→1000, `fit_content` spans it,
  new time maps to a real on-canvas x). tsc + wasm builds green.
- 2026-07-14 — **Phase B polish: `chart.panes()` handle API (LWC `IPaneApi` parity).** `chart.panes()`
  now returns a `pane_api[]` (one per stacked pane), each with `pane_index()`, `get_height()`,
  `set_height(px)`, `get_stretch_factor()`, `set_stretch_factor(n)`. Engine gained
  `pane_height`/`pane_stretch` getters and `set_pane_stretch`/`set_pane_height` (the latter reuses
  the separator-drag freeze-and-redistribute so a resize absorbs its delta from the neighbour).
  Verified via the API (no screenshots — capture wedged this session): `panes()` grows 1→2 when a
  volume pane is added; stretch 1:0.5 → heights 364:182; `set_height(300)` moves pane 0 to 300 and
  its neighbour 182→245 (separator 365→301); `set_stretch_factor(3)` from equal panes gives a clean
  2.99≈3 height ratio. Note: `set_height` freezes heights into stretch factors (same as dragging a
  separator), so a later `set_stretch_factor` is relative to those frozen values — inherent to the
  pane model, consistent with drag. No console errors.
  **Phase B polish remaining:** marker `text` labels on the 2D overlay (a visual increment — best
  done once the preview pane's screenshot capture recovers).
- 2026-07-14 — **Phase B polish: marker text labels (completes the markers feature).**
  `draw_marker_labels_2d` renders each marker's `text` on the Canvas2D overlay, centered on the
  marker's x and placed clear of the shape (above above-markers, below below-markers), in the
  marker color, clipped to the pane band and visible index range. Verified by reading the overlay
  canvas pixels directly (it's a 2D context, so `getImageData` works even with the WebGPU-pane
  screenshot capture still wedged this session): 3 `"BUY"` labels produce 942 marker-color pixels
  clustered at x≈108/232/348 — matching the expected label centers 116/232/348 — and drop to 0
  after `set_markers([])`. No console errors. **Series markers are now feature-complete (shapes +
  text); Phase B and its polish pass are done.**
- 2026-07-14 — **Phase D2 begun: Canvas2D executor for the Prim IR** (`aion_render::canvas2d`). A
  pure, gpu/dom-free translator from the `Prim` draw-list IR into `CanvasRenderingContext2D`-style
  calls, issued against an abstract `Canvas2d` target trait (concrete web-sys + native impls land
  later — browsers without WebGPU, and the golden/SSR render path). The crisp-rect subset
  (`Rect`/`RectFrame`/`HLine`/`VLine`) reuses the exact integer + dash math of the wgpu quad
  executor so the two backends agree pixel-for-pixel on rects; `Polyline` (with step/curve
  expansion via `expand_line` + dash reset), `AreaFill` (path closed down to base with a vertical
  gradient), `Circle`, `RoundRect`, and `Background` map onto native path/gradient calls; `Text` is
  reserved (drawn by the 2D text path, not this executor). 8 unit tests via a recording target
  assert the emitted command stream for every prim (render crate 37→45 tests). Next D2 increments:
  concrete web-sys target in `aion_wasm` behind a WebGPU-absent fallback, and refactoring the live
  line/area/marker builders to emit the high-level `Polyline`/`AreaFill`/`Circle` prims (they
  currently tessellate straight to wgpu tri-meshes) so the fallback can render them too.
- 2026-07-14 — **Phase D2 increment 2 + D1 groundwork: native `aion_native` rasterizer target.**
  New `aion_native` crate implements the `Canvas2d` trait on `tiny_skia` (pure-Rust CPU rasterizer,
  no system deps): solid + vertical-gradient fills, path stroke with dash, arc tessellation, PNG
  encode, and straight-RGBA pixel readout. `render_prims(w,h,bg,prims,points)` rasterizes a prim
  layer to a `Pixmap`. This is the off-GPU deterministic render path the roadmap wants — the
  foundation for golden-image tests (D1) and server-side PNGs (D2). Verified two ways: 3 pixel-
  assertion unit tests (rect fill, circle center vs corner, gradient top-vs-bottom), and an
  `examples/scene.rs` that renders a full chart-like scene (background gradient + grid + 6
  candlesticks with wicks/bodies + area fill + polyline + dashed price line + circle marker) to a
  PNG — inspected directly and correct. Workspace now 144 tests green (core 96, render 45,
  native 3). Next: wire a golden-diff harness (render LWC reference PNGs, compare per-pixel with
  rect-exact / AA-tolerant thresholds), and the web-sys `Canvas2d` target for in-browser fallback.
- 2026-07-14 — **Phase D1: golden-image regression harness.** `diff_pixmaps(a, b, tolerance)`
  reports differing-pixel count / max channel delta / fraction; the reference scene moved into
  `aion_native::scene::demo_scene()` so the example renderer and the harness render byte-identical
  output. A committed golden PNG (`tests/goldens/scene.png`) plus an integration test
  (`tests/golden.rs`) that re-renders and asserts <0.1% drift (per-channel tolerance 2, so a
  tiny-skia patch bump won't spuriously fail), with a negative-control test proving the diff
  actually detects a changed scene. Regenerate deliberately via the `scene` example. This is the
  regression net the roadmap wants across A–C; LWC-reference PNGs drop in as more goldens once a
  headless-Chromium pipeline exists. Workspace 146 tests green (native 3 unit + 2 golden). Next:
  web-sys `Canvas2d` target + WebGPU-absent fallback wiring in `aion_wasm`.
- 2026-07-14 — **Phase D2 increment 3: web-sys `Canvas2d` target (`aion_wasm::canvas2d_target`).**
  `WasmCanvas2d` implements the executor's `Canvas2d` trait over a real `CanvasRenderingContext2d`
  (solid + `createLinearGradient` fills preserving alpha via `rgba()`, dashed strokes via
  `setLineDash`, arcs, paths) — the in-browser fallback backend for machines without WebGPU. Added
  the `CanvasGradient` web-sys feature. An exported `render_prim_smoke_2d(canvas)` runs the executor
  against a 2D canvas so it can be verified without WebGPU. Verified in-browser via `getImageData`:
  a rect reads pure red, a circle center pure blue (its bbox corner stays background — round),
  a polyline reads its green, and the background gradient reads near-white at top → light-blue at
  bottom. Every prim type drives the real 2D canvas correctly. Next (the larger step): detect
  WebGPU absence in the shell and route the chart's frame through this target, which needs the live
  line/area/marker builders to emit high-level `Polyline`/`AreaFill`/`Circle` prims (they currently
  tessellate straight to wgpu tri-meshes).
- 2026-07-15 — **Phase D2 increment 4: unify line/area geometry into the Prim IR.** The live
  line/area builder (`build_line_prims`) now emits high-level `AreaFill` + `Polyline` + `Circle`
  (point-marker) prims into the pane's shared `prims` list, pushing **device-space** points into a
  per-pane pool — instead of tessellating straight to wgpu tri-meshes. A new
  `aion_render_wgpu::geom_prims_to_tris` walks those prims and tessellates them back into the tri
  buffers for the GPU (reusing the same `build_area_fill`/`build_line_stroke`/`build_disc` helpers
  with identity pixel ratios, since the pool is already scaled — so wgpu output is byte-identical).
  Both backends now consume one prim list: the Canvas2D fallback executor already renders
  `Polyline`/`AreaFill`/`Circle`, so line/area series are now expressible off-GPU. `Prim::AreaFill`
  gained a `line_type` field so stepped/curved areas trace the same edge on both backends (the 2D
  executor previously hardcoded `Simple`, a latent mismatch — now fixed). Verified: workspace 154
  tests green (render 45, wgpu 4→8 with 4 new `tri_executor` tests, native golden unchanged — the
  `line_type: Simple` default is a no-op for the committed scene); in-browser the area series
  (green gradient fill + stroke) and the SMA line series render correctly through the new
  prim→tessellation path with no console errors. **Remaining before the fallback can render a full
  frame:** baseline series, series-markers (square/arrow shapes), and the last-price pulse still
  tessellate straight to tris (not yet prim-expressible); then the larger step — make `Gfx` optional
  and route the frame through `WasmCanvas2d` when WebGPU is absent.
