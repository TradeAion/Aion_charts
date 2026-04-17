# Indicator Runtime

AxiusCharts includes a user-indicator pipeline that compiles source into an intermediate representation, attaches runtime instances, and emits draw instructions and diagnostics back into the chart engine.

## Runtime Overview

- Source is compiled into indicator IR on the main thread
- The runtime attaches indicator instances with input bindings and resource limits
- Instances can consume MTF snapshots and emit diagnostics, stats, and runtime events
- Render-facing output is converted into chart draw instructions and pane data

## Resource Limits

`ResourceLimits` and `ResourceCounters` define the runtime budget contract:

- instruction limits
- memory-like object or array limits
- runtime accounting surfaced through indicator stats

Any new runtime feature should integrate with those counters instead of bypassing them.

## MTF Resolver Contract

The MTF resolver interface is responsible for:

- declaring requested higher-timeframe series
- accepting externally supplied snapshots
- feeding those snapshots into the runtime without changing the chart core's time-scale contract

This keeps MTF work explicit and testable rather than hiding it inside the renderer or viewport.

## Worker Offload Plan

### Why It Matters

User-authored indicators can become expensive enough to block the browser main thread. Rendering, input, and layout all compete for that same thread today.

### Proposed Architecture

1. Compile indicator IR on the main thread
2. Ship IR, resource limits, and data frames to a Web Worker
3. Run the interpreter in the Worker
4. Return `DrawInstruction[]`, diagnostics, and counters to the main thread
5. Merge those results into the existing pane/render pipeline

### Message Protocol Sketch

- `CompileIndicatorRequest` -> main thread to Worker
- `AttachIndicatorInstance` -> main thread to Worker
- `PushFrameBatch` -> main thread to Worker
- `SetMtfSnapshot` -> main thread to Worker
- `IndicatorFrameResult` -> Worker to main thread
- `IndicatorDiagnostics` -> Worker to main thread
- `IndicatorRuntimeFault` -> Worker to main thread

### Migration Triggers

Turn on Worker execution when one or more of these are true:

- many indicator instances are active at once
- user scripts are large or loop-heavy
- profiling or user reports show scroll or crosshair jank
- a hosted environment requires stronger isolation for untrusted scripts

The important constraint is that Worker offload must remain an internal runtime choice, not a public API fork.
