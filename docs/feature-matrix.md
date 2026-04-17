# Feature Matrix

Last verified: 2026-04-17 against commit 97ae55b

## Series Types

| Area | Status | Notes |
| --- | --- | --- |
| Main candlestick chart | Implemented | Core primary mode |
| Main OHLC chart | Implemented | Alias: `bars` |
| Main line chart | Implemented | Primary close/value line |
| Main area chart | Implemented | Filled line variant |
| Main Heikin-Ashi chart | Implemented | Derived from source bars |
| Main baseline chart | Implemented | Baseline shading supported |
| Main footprint chart | Implemented | Bid/ask, delta, and profile modes |
| Overlay line series | Implemented | Separate series collection |
| Overlay area series | Implemented | Separate series collection |
| Overlay histogram series | Implemented | Color-per-point support |
| Overlay bar series | Implemented | OHLC overlay path |
| Overlay baseline series | Implemented | Base value support |

## Multi-Pane And Groups

| Area | Status | Notes |
| --- | --- | --- |
| Indicator sub-panes | Implemented | Add, update, remove |
| Pane separators | Implemented | Draggable sizing |
| Chart groups | Implemented | Shared symbol, interval, range, and crosshair syncing |
| Split workspaces | Implemented | Managed through group/workspace APIs |

## Drawings

| Area | Status | Notes |
| --- | --- | --- |
| Trend line | Implemented | |
| Horizontal line | Implemented | |
| Vertical line | Implemented | |
| Ray | Implemented | |
| Rectangle | Implemented | |
| Fibonacci | Implemented | |
| Scale | Implemented | |
| Brush | Implemented | |
| Drawing snapshot versioning | Implemented | `DRAWINGS_SNAPSHOT_VERSION` plus migration entry point |

## Indicators And Studies

| Area | Status | Notes |
| --- | --- | --- |
| Built-in studies | Implemented | SMA, EMA, RSI, MACD, Bollinger, Stochastic, ATR, VWAP |
| User-indicator compiler/runtime | Implemented | IR compiler and runtime APIs present |
| MTF snapshots | Implemented | Explicit runtime inputs |
| Worker offload | Planned | Documented in `indicator-runtime.md` |

## Price Scales And Axes

| Area | Status | Notes |
| --- | --- | --- |
| Normal scale | Implemented | |
| Logarithmic scale | Implemented | |
| Percentage scale | Implemented | |
| Indexed-to-100 scale | Implemented | |
| Price axis labels | Implemented | Logical price domain stays `f64` until projection |
| Time axis merged-slot indexing | Implemented | Shared `TimeScaleIndex` contract |

## Crosshair And Events

| Area | Status | Notes |
| --- | --- | --- |
| Crosshair hover and snapping | Implemented | |
| Click events | Implemented | |
| `visibleRangeChange` during gestures | Implemented | Includes kinetic glide |
| Drawing events | Implemented | |
| Execution mark events | Implemented | Hover and click |

## Theming And Data Input

| Area | Status | Notes |
| --- | --- | --- |
| Dark/light presets | Implemented | |
| Custom theme objects | Implemented | |
| CSS variable export | Implemented | |
| OHLCV typed-array input | Implemented | `Float64Array` + `BigUint64Array` |
| Footprint typed-array input | Implemented | `Float64Array` levels and volumes |
| JSON persistence import/export | Implemented | |

## Renderer

| Area | Status | Notes |
| --- | --- | --- |
| WebGPU backend | Implemented | Primary renderer |
| Canvas2D backend | Implemented | Fallback renderer |
| Backend parity harness | Partial | Structural parity harness exists; pixel diff is future work |
| Shader precision changes | Won't-do | WGSL vertex attribute layout remains single-precision by design |
