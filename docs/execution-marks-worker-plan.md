# Execution Marks Worker Offload Plan

This document is the design seam for moving execution-mark hit-testing off the main thread if dense fill workloads start to cost frames. It does **not** change the current runtime: rendering stays on the main thread today.

## Thresholds

Measured on the current Windows development machine with the pure `hit_test_execution_mark_hit_areas(...)` helper in release mode. Treat these as the baseline to compare against target user hardware before enabling any Worker path.

| Mark count | p50 hovered hit-test | p95 hovered hit-test |
| --- | ---: | ---: |
| 1,000 | 0.225 µs | 0.245 µs |
| 10,000 | 2.777 µs | 3.525 µs |
| 50,000 | 17.902 µs | 20.953 µs |
| 100,000 | 41.605 µs | 47.173 µs |

Interpretation:

- 1k to 10k marks are comfortably main-thread safe.
- 50k marks are still workable but are now visible in per-mousemove budgets.
- 100k marks are the point where main-thread hover cost becomes material, especially when combined with layout, event dispatch, and frontend work.

## Boundary

Move to Worker:

- the resolved execution-mark hit-area vector for the current frame
- a spatial index built over `(x_css, y_css)` keyed by `mark_id`
- point-query execution for hover/click lookup

Keep on main:

- render passes
- WebGPU command encoding
- Canvas2D drawing
- event emission
- selected-state mutation
- the final decision to invalidate and draw a new frame

The Worker is for hit-test acceleration only. Rendering remains on the main thread.

## Message Protocol

Suggested message flow:

```ts
// main -> worker
{ type: 'update_index', revision, hitAreas }
{ type: 'query', id, x, y }

// worker -> main
{ type: 'ready', revision }
{ type: 'result', id, mark_id?: string }
```

Rules:

- `id` is a monotonic query ID from the main thread
- the main thread drops stale results when `result.id < latestIssuedId`
- `revision` increments whenever the hit-area dataset changes
- the Worker ignores `query` requests for stale revisions

## Data Structure

Recommended first implementation:

- uniform grid over `(x_css, y_css)` cells keyed by `mark_id`
- each cell stores a compact list of candidate hit areas
- query checks the cell under the pointer plus immediate neighbors

Alternative:

- k-d tree if mark distribution becomes extremely uneven

Rebuild triggers:

- `resolve_time_scale_indices(...)` / visible-range changes that move execution marks in X
- viewport price changes that move execution marks in Y
- mark add/remove/set/clear operations
- cluster-threshold changes, because clusters alter the hit-area set

## Debounce Strategy

Mousemove queries must remain lossy:

- issue a new monotonic `query.id` on every pointer move
- keep only the latest pending query on the main thread
- discard any Worker reply older than the last issued ID

This avoids backpressure and prevents stale hover state from flashing after a fast pan or zoom.

## Fallback

If Worker support is unavailable or not desirable:

- keep the current main-thread path
- continue using the existing hit-area vector reverse scan
- do not add product-level feature gating for hosts

The offload decision should remain an internal engine implementation detail.

## Out Of Scope

Not part of this plan:

- rendering in a Worker
- moving WebGPU or Canvas2D commands off the main thread
- changing public execution-mark APIs
- changing the event contract

Future implementation work should preserve the current public API and keep rendering on the main thread.
