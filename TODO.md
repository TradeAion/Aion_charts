# AxiusCharts Modernization — Production-Grade API Refactoring

> Started: 2026-02-26 | Status: **ALL PHASES COMPLETE + VERIFIED (final pass)**
> Previous phases (P0-P3, Phase 5) were COMPLETE — see git history.
> This document tracks the **API Modernization** effort.
> Last updated: 2026-04-17 by migration hardening pass.

---

## Quick Context for Continuation Agents

**What we did:** Complete modernization of AxiusCharts's public API, theming, event system, and DX.

**Architecture:** Two Rust crates:
- `axiuscharts` (src/) — platform-agnostic core engine, renderers, data structures
- `axiuscharts-wasm` (wasm/src/) — thin WASM/DOM interop layer via wasm-bindgen

**Build commands:**
```bash
cargo check                                                    # core (0 warnings)
cargo check --target wasm32-unknown-unknown -p axiuscharts-wasm    # wasm (0 warnings)
cargo test                                                     # 98 unit + 4 doc tests
wasm-pack build wasm --target web --release                    # build WASM package → wasm/pkg/
```

**Verified state (2026-02-26 — final):**
- `cargo test` → 98 unit + 4 doc tests pass
- Both crates compile with **0 warnings, 0 errors**
- `wasm-pack build` succeeds (886KB .wasm, 156KB .js)
- All new API methods visible in generated JS: `create_chart`, `apply_options`, `on`, `off`, `once`, `start_auto_render`, `stop_auto_render`, `is_auto_render`, `get_css_variables`, `theme`
- **Critical RAF loop bug fixed** (was only firing once, now loops correctly)
- `wasm/axiuscharts.d.ts` — hand-crafted TypeScript definitions with full types for events, options, theme

---

## Public Contract Alignment Sweep

- [x] Fix selector-vs-ID semantics in `create_chart` examples and docs
- [x] Align `ChartGroup` docs/example with runtime signatures
- [x] Align price-line and marker docs with runtime signatures
- [x] Normalize event payload field names in docs/examples
- [x] Re-audit the remaining docs for any stale snake_case field names

## 2026-03-27 Interaction/Event Alignment

- [x] Make mouse double-click reset distance-aware instead of zone-only
- [x] Make touch double-tap reset distance-aware before triggering chart reset
- [x] Keep click bar-index lookup aligned with hover/crosshair slot math
- [x] Emit `visibleRangeChange` from chart drag, wheel, pinch, and double-click paths
- [x] Wire draggable price-line interaction at the chart pointer layer
- [x] Follow through on the RAF-driven gliding path so momentum scrolls can emit `visibleRangeChange` too

## Post-Migration Closure (2026-04-17)

- [x] Move logical price storage, persistence, indicators, and WASM typed-array inputs to `f64`
- [x] Remove `Bar::_pad`, document the real `Bar` layout, and add `Bar::new(...)`
- [x] Replace the stale feature audit with `docs/feature-matrix.md`
- [x] Add version-aware drawing snapshot migration scaffolding via `migrate_snapshot(...)`
- [x] Add a structural backend parity harness behind `--features parity-tests`
- [x] Restructure repo documentation around architecture, price domain, events, testing, persistence, and performance

---

## Phase Completion Summary

| Phase | Description | Status | Files Changed |
|-------|-------------|--------|---------------|
| 1 | ThemeConfig system (core) | [x] DONE | `src/core/renderer/theme.rs`, `src/lib.rs` |
| 2 | Core Event System | [x] DONE | `src/core/events.rs`, `src/core/engine.rs`, `src/lib.rs` |
| 3 | Eliminate hardcoded colors | [x] DONE | ~20 files (drawings, series, chart_type, markers, price_line, overlay, price_axis, subpane, workspace, wasm/lib) |
| 4 | Modern create_chart() + options | [x] DONE | `wasm/src/lib.rs` |
| 5 | Event Emitter WASM bridge | [x] DONE | `wasm/src/event_emitter.rs` (NEW), `wasm/src/lib.rs` |
| 6 | Auto-Render RAF loop | [x] DONE + FIXED | `wasm/src/render_frame.rs` (NEW), `wasm/src/lib.rs` |
| 7 | CSS Variable Output | [x] DONE | `src/core/renderer/theme.rs`, `wasm/src/lib.rs` |
| 8 | Legacy Deprecation Bridge | [x] DONE | `wasm/src/lib.rs` |
| 9a | Wire events into handlers | [x] DONE | `wasm/src/chart_inner.rs`, `wasm/src/lib.rs` |
| 9b | API docs + Leptos example | [x] DONE | `TODO.md` |
| 9c | **demo/index.html rewrite** | [x] DONE | `demo/index.html` — modern API, theme toggle, event HUD |
| 9d | **Fix RAF render loop bug** | [x] DONE | `wasm/src/render_frame.rs` — extracted do_render_frame() |
| 9e | **Zero compiler warnings** | [x] DONE | Various files — unused imports/variables suppressed |
| 9f | **Fix RAF self-reference bug** | [x] DONE | `wasm/src/lib.rs` — loop now reschedules correctly (Rc slot fix) |
| 9g | **Hand-crafted TypeScript defs** | [x] DONE | `wasm/axiuscharts.d.ts` — full types for events, options, theme |

## Critical Bug Fixed in Final Session — RAF Self-Reference

The auto-render RAF loop had a fatal bug: the Closure was stored into the
shared `Rc<RefCell<Option<Closure>>>` slot **and then immediately `.take()`-n
out** of it. This left `slot_for_reschedule` (captured inside the closure)
pointing to `None`, so the loop fired exactly once and stopped.

**Fix:** Changed `_raf_closure` to `Option<Rc<RefCell<Option<Closure<dyn FnMut()>>>>>`.
The Closure now stays in the slot. `stop_auto_render_internal` calls
`slot.borrow_mut().take()` to break the reference cycle before dropping the Rc.

---

## Critical Bug Fixed (Phase 6 Correction)

The auto-render RAF loop was **not actually rendering**. The loop scheduled itself but only
did DPR detection — it never called the render pipeline.

**Fix:** Extracted the full render body into `wasm/src/render_frame.rs::do_render_frame()`.
Both `AxiusCharts::render()` (public API) and the RAF closure now delegate to this free function.
The `event_emitter` field was changed from `EventEmitter` to `Rc<RefCell<EventEmitter>>`
so the RAF closure can capture it.

```
wasm/src/render_frame.rs  (NEW) — do_render_frame(inner, dirty, event_emitter)
wasm/src/lib.rs           — RAF closure now calls do_render_frame()
                          — event_emitter: Rc<RefCell<EventEmitter>>
                          — flush_events() now uses borrow() not &mut self
```

---

## New Public API Reference

### Creating a Chart

```javascript
// Modern API (recommended)
const chart = await AxiusCharts.create_chart(
  document.getElementById('chart'),  // HTMLElement or string ID
  {
    theme: 'dark',                   // 'dark' | 'light'
    renderer: 'auto',               // 'auto' | 'webgpu' | 'canvas2d'
    autoRender: true,                // auto requestAnimationFrame loop
    symbol: 'BTCUSD',
    interval: '1D',
    watermark: 'AxiusCharts',
    crosshair: { mode: 'normal' },   // 'normal' | 'magnet_ohlc'
    priceScale: {
      mode: 'normal',               // 'normal' | 'logarithmic' | 'percentage' | 'indexedTo100'
      margins: { top: 0.1, bottom: 0.1 }
    }
  }
);

// Legacy API (deprecated, still works)
const chart = await AxiusCharts.create('container-id');
```

### Event System

```javascript
// Subscribe to events
chart.on('crosshairMove', (e) => {
  console.log(e.x, e.y, e.price, e.barIndex, e.timestamp);
});

chart.on('click', (e) => {
  console.log('Clicked at', e.x, e.y, 'price:', e.price);
});

chart.on('visibleRangeChange', (e) => {
  console.log('Range:', e.startBar, '-', e.endBar);
});

chart.on('symbolChange', (e) => console.log('Symbol:', e.symbol));
chart.on('intervalChange', (e) => console.log('Interval:', e.interval));
chart.on('chartTypeChange', (e) => console.log('Type:', e.chartType));
chart.on('priceScaleChange', (e) => console.log('Scale:', e.mode));
chart.on('resize', (e) => console.log('Size:', e.width, e.height));
chart.on('error', (e) => console.error(e.message));

// One-shot listener
chart.once('click', (e) => console.log('First click!'));

// Unsubscribe
chart.off('crosshairMove', myCallback);
```

### Available Events

| Event Name | Payload Fields | Fires When |
|-----------|---------------|------------|
| `crosshairMove` | `x, y, barIndex, price, timestamp` | Mouse moves over chart |
| `click` | `x, y, barIndex, price` | Non-drag click on chart |
| `visibleRangeChange` | `startBar, endBar` | Zoom/pan/set_visible_range |
| `symbolChange` | `symbol` | set_symbol() called |
| `intervalChange` | `interval` | set_interval() called |
| `chartTypeChange` | `chartType` | set_chart_type() called |
| `priceScaleChange` | `mode` | set_price_scale_mode() called |
| `resize` | `width, height` | Container resized |
| `error` | `message` | Error during operation |

### Runtime Options Update

```javascript
// Switch to light theme at runtime
chart.apply_options({ theme: 'light' });

// Change multiple settings
chart.apply_options({
  watermark: 'My Chart',
  crosshair: { mode: 'magnet_ohlc' },
  symbol: 'ETHUSD',
});
```

### Theme System

```javascript
// Get current theme
chart.theme();  // 'dark' | 'light' | 'custom'

// Get CSS variables (for framework integration)
const vars = chart.get_css_variables();
// { '--axiuscharts-bg': 'rgba(23,23,23,1)', '--axiuscharts-bullish': '...', ... }
```

CSS variables set on the container element:
```css
--axiuscharts-bg, --axiuscharts-text, --axiuscharts-bullish, --axiuscharts-bearish,
--axiuscharts-grid, --axiuscharts-border, --axiuscharts-watermark,
--axiuscharts-crosshair, --axiuscharts-crosshair-label-bg, --axiuscharts-crosshair-label-text,
--axiuscharts-font-family, --axiuscharts-font-size
```

### Auto-Render Control

```javascript
chart.start_auto_render();   // Enable auto RAF loop
chart.stop_auto_render();    // Disable (manual render() needed)
chart.is_auto_render();      // Check status
chart.render();              // Manual render (always works)
```

### Cleanup

```javascript
chart.dispose();  // Remove all listeners, stop RAF, clean up DOM
```

---

## Leptos Integration Example

```rust
use leptos::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = AxiusCharts)]
    async fn create_chart(container: &JsValue, options: &JsValue) -> JsValue;
}

#[component]
pub fn Chart(
    #[prop(default = "dark".to_string())] theme: String,
    #[prop(default = "BTCUSD".to_string())] symbol: String,
) -> impl IntoView {
    let container_ref = create_node_ref::<html::Div>();
    let chart_handle = create_rw_signal::<Option<JsValue>>(None);

    // Mount chart when DOM is ready
    create_effect(move |_| {
        if let Some(el) = container_ref.get() {
            let theme = theme.clone();
            let symbol = symbol.clone();
            spawn_local(async move {
                let options = js_sys::Object::new();
                js_sys::Reflect::set(&options, &"theme".into(), &theme.into()).unwrap();
                js_sys::Reflect::set(&options, &"symbol".into(), &symbol.into()).unwrap();
                js_sys::Reflect::set(&options, &"autoRender".into(), &true.into()).unwrap();

                let chart = create_chart(&el.into(), &options.into()).await;
                chart_handle.set(Some(chart));
            });
        }
    });

    // Cleanup on unmount
    on_cleanup(move || {
        if let Some(chart) = chart_handle.get_untracked() {
            let _ = js_sys::Reflect::apply(
                &js_sys::Reflect::get(&chart, &"dispose".into()).unwrap().into(),
                &chart,
                &js_sys::Array::new(),
            );
        }
    });

    // Reactive theme switching via CSS variables
    // (theme changes are picked up via --axiuscharts-* vars on the container)

    view! {
        <div
            node_ref=container_ref
            style="width: 100%; height: 400px; position: relative;"
        />
    }
}

// Usage in a Leptos app:
// <Chart theme="dark" symbol="BTCUSD" />
// <Chart theme="light" symbol="ETHUSD" />
```

### Leptos Integration Tips

1. **CSS Variables**: The chart sets `--axiuscharts-*` CSS variables on its container. Reference these in Tailwind:
   ```css
   .chart-tooltip { background: var(--axiuscharts-bg); color: var(--axiuscharts-text); }
   ```

2. **Reactive Options**: Use `apply_options()` when Leptos signals change:
   ```rust
   create_effect(move |_| {
       if let Some(chart) = chart_handle.get() {
           let opts = js_sys::Object::new();
           js_sys::Reflect::set(&opts, &"symbol".into(), &symbol.get().into()).unwrap();
           js_sys::Reflect::apply(
               &js_sys::Reflect::get(&chart, &"apply_options".into()).unwrap().into(),
               &chart, &js_sys::Array::of1(&opts.into()),
           ).unwrap();
       }
   });
   ```

3. **Events → Signals**: Wire chart events to Leptos signals:
   ```rust
   let crosshair_price = create_rw_signal(0.0_f64);
   // In the mount effect:
   let on_move = Closure::wrap(Box::new(move |e: JsValue| {
       if let Some(price) = js_sys::Reflect::get(&e, &"price".into()).ok().and_then(|v| v.as_f64()) {
           crosshair_price.set(price);
       }
   }) as Box<dyn FnMut(JsValue)>);
   js_sys::Reflect::apply(
       &js_sys::Reflect::get(&chart, &"on".into()).unwrap().into(),
       &chart, &js_sys::Array::of2(&"crosshairMove".into(), on_move.as_ref().unchecked_ref()),
   ).unwrap();
   on_move.forget(); // prevent drop
   ```

---

## Architecture Overview (Post-Modernization)

```
┌─────────────────────────────────────────────────────┐
│  Consumer (JS / Leptos / React / etc.)               │
│  const chart = await AxiusCharts.create_chart(el, opts)  │
│  chart.on('crosshairMove', fn)                       │
│  chart.apply_options({ theme: 'light' })             │
└──────────┬──────────────────────────────────────┬────┘
           │ wasm_bindgen                          │
┌──────────▼──────────────────────────────────────▼────┐
│  wasm/src/lib.rs  (AxiusCharts struct)                   │
│  ├── EventEmitter (on/off/once → js_sys::Function)   │
│  ├── ThemeConfig (Dark/Light/Custom)                 │
│  ├── CreateChartOptions (JsValue parsing)            │
│  ├── Auto-RAF loop (dirty-flag based)                │
│  ├── applyOptions() (partial merge)                  │
│  ├── CSS Variable output to container                │
│  └── Legacy aliases with deprecation warnings        │
├──────────────────────────────────────────────────────┤
│  wasm/src/event_emitter.rs                           │
│  └── EventEmitter + chart_event_to_js() converter    │
├──────────────────────────────────────────────────────┤
│  wasm/src/chart_inner.rs                             │
│  └── Event emission (crosshairMove, click, range)    │
└──────────┬───────────────────────────────────────────┘
           │
┌──────────▼───────────────────────────────────────────┐
│  src/core/ (platform-agnostic engine)                │
│  ├── renderer/theme.rs — ThemeConfig + presets        │
│  ├── events.rs — ChartEvent enum + EventBus          │
│  ├── engine.rs — ChartEngine (owns EventBus)         │
│  ├── drawings/* — colors from ThemeConfig             │
│  ├── series/* — defaults from ThemeConfig             │
│  └── renderer/* — uses ChartStyle (from ThemeConfig)  │
└──────────────────────────────────────────────────────┘
```

---

## Files Created or Modified

### New Files
- `wasm/src/event_emitter.rs` — JS event emitter (on/off/once, chart_event_to_js)
- `wasm/src/render_frame.rs` — Extracted render pipeline (do_render_frame free function)
- `wasm/axiuscharts.d.ts` — Hand-crafted TypeScript definitions with full event/option types

### Major Rewrites
- `src/core/events.rs` — ChartEvent enum (11 variants) + EventBus (ring buffer)
- `src/core/renderer/theme.rs` — ThemeConfig system (Dark/Light presets, sub-structs, CSS vars)

### Modified (event wiring + theme integration)
- `src/core/engine.rs` — Added EventBus field
- `src/lib.rs` — Added re-exports for ThemeConfig and events
- `wasm/src/lib.rs` — Modern API (create_chart, apply_options, on/off/once, auto-render, deprecation bridge, CSS vars)
- `wasm/src/chart_inner.rs` — Event emission (crosshairMove, click, visibleRangeChange)

### Modified (hardcoded color removal → ThemeConfig)
- `src/core/drawings/types.rs` — DrawingStyle::from_theme(), rectangle_from_theme(), fibonacci_from_theme(), scale_from_theme()
- `src/core/drawings/mod.rs` — default_anchor_color()
- `src/core/drawings/trend_line.rs`, `rectangle.rs`, `fibonacci.rs`, `scale.rs` — use DrawingStyle constructors
- `src/core/drawings/drawing.rs`, `ray.rs`, `vertical_line.rs`, `horizontal_line.rs` — anchor colors from theme
- `src/core/series/line_options.rs`, `area_options.rs`, `bar_options.rs`, `baseline_options.rs`, `histogram_options.rs` — defaults from ThemeConfig
- `src/core/chart_type.rs` — MainChartOptions defaults from ThemeConfig
- `src/core/markers.rs` — marker colors from ThemeConfig
- `src/core/price_line.rs` — price line colors from ThemeConfig
- `src/core/renderer/overlay.rs` — font from theme constant
- `src/core/renderer/price_axis.rs` — text color from theme
- `wasm/src/subpane.rs` — indicator colors from ThemeConfig palette, font from theme
- `wasm/src/workspace.rs` — workspace colors from ThemeConfig

---

*Completed: 2026-02-26 — ALL MODERNIZATION PHASES DONE (RAF bug fixed, TypeScript defs added)*
