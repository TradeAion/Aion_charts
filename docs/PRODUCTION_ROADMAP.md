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
