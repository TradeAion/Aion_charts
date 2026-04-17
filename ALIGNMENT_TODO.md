# AxiusCharts Alignment Todo

## Landed

- [x] Keep exact device-pixel widget sizing active after every render pass.
- [x] Avoid heavy render work when the chart is not dirty and no animation/replay is active.
- [x] Mark chart state dirty from the main interaction and resize paths that mutate viewport or crosshair state.
- [x] Emit `visibleRangeChange` from pane drag/wheel, price axis drag/wheel, time axis drag/wheel, pinch, double tap, and glide updates.
- [x] Compare live-update overflow checks in internal price-scale space so percentage/log/indexed modes do not jitter or miss refits.
- [x] Use distance thresholds for double-click/double-tap reset detection instead of time-only matching.
- [x] Stop right-gap crosshair labels from falling back to `"0"` when no timestamp is available.
- [x] Draw axis tick marks consistently and use major/minor time-axis label styling based on actual label granularity.
- [x] Align percentage/indexed/log price-scale math more closely with Lightweight Charts, including negative-base handling.
- [x] Improve price precision inference so overlays are not locked to a `0.01` assumption.
- [x] Align public docs and TypeScript declarations with runtime behavior and event payload shapes.
- [x] Wire draggable price-line hit testing and drag updates into pointer handling.
- [x] Include visible overlay series and overlay base values in unlocked autoscale.
- [x] Re-fit unlocked price range immediately when overlay data, visibility, or membership changes.
- [x] Reject non-finite main-bar and overlay inputs at the engine/API path instead of silently converting them to viewport-corrupting zeroes.
- [x] Make drawing hit-testing follow the same bucketed paint order as rendering, with later-painted drawings winning equal-distance ties.
- [x] Use fractional DOM rect sizing for subpane canvases before DPR rounding to reduce blur on fractional layouts.
- [x] Convert auto-render from a permanent RAF loop into invalidation-driven one-shot scheduling, while still re-queuing during glide, replay, and subpane kinetic animation.
- [x] Share timestamp-to-logical-index math across overlay geometry paths instead of keeping a separate line-series mapper.
- [x] Include visible overlay timestamps in time-axis tick labels and crosshair time labels, while still falling back to whitespace extrapolation in the gaps.
- [x] Add device-pixel-content-box-aware exact sizing for subpane chart and axis canvases instead of relying only on rounded CSS sizes.
- [x] Reject non-finite writes inside the low-level series and main-bar storage arrays instead of silently sanitizing them to zero, while preserving finite OHLC/volume normalization.
- [x] Build one shared per-frame `TimeScaleIndex` and reuse it across renderer, overlay, and execution-mark timestamp lookups instead of rebuilding ad hoc timestamp vectors.
- [x] Promote the shared `TimeScaleIndex` into the main time-axis contract so main-series geometry, markers, crosshair snapping, synced crosshair projection, execution marks, and subpane indicator lines all resolve through merged logical slots instead of raw bar indices.
- [x] Keep public `barIndex` APIs anchored to real main bars while letting internal rendering/event timestamp paths derive overlay-only logical positions from the merged time scale.
- [x] Keep execution-mark rendering role-aware, cluster-aware, and snapshot-versioned without regressing the shared time-scale contract or reintroducing grouped-fill connector lines.
- [x] Run a browser smoke pass covering page load, pointer drag, wheel zoom, drawing gesture, and viewport resize on the demo app without new console/runtime errors.
- [x] Extend the browser smoke pass to cover merged-slot-sensitive behavior: wheel zoom, pan, and subpane creation on the demo app without new console/runtime errors.
- [x] Re-audit the remaining docs for stale snake_case public JS event payload names and align them to camelCase.
