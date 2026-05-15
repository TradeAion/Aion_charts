# Aion_charts Docs

This directory is the working documentation set for the Aion_charts repository. It tracks the current Rust core, the WASM bridge, and the public JS contract.

| Document | Purpose |
| --- | --- |
| [getting-started.md](./getting-started.md) | Install, initialize WASM, create a chart, and load `Float64Array` data correctly. |
| [api-reference.md](./api-reference.md) | Public API categories, runtime guarantees, and links to the exact TypeScript contract. |
| [framework-guide.md](./framework-guide.md) | React, Vue, Svelte, vanilla JS, and bundler integration patterns. |
| [theming.md](./theming.md) | Theme presets, custom theme objects, CSS variables, and targeted visual overrides. |
| [drawing-tools.md](./drawing-tools.md) | Interactive drawing tools, workflow, keyboard handling, and persistence entry points. |
| [execution-marks.md](./execution-marks.md) | Execution-mark data model, label modes, P&L rendering, clustering, selection, and events. |
| [execution-marks-persistence.md](./execution-marks-persistence.md) | Versioned execution-mark snapshot shape, import compatibility, and migration policy. |
| [execution-marks-worker-plan.md](./execution-marks-worker-plan.md) | Worker-offload plan for high-volume execution-mark hit-testing and its measured thresholds. |
| [architecture.md](./architecture.md) | Crate layout, module map, data flow, event flow, invalidation pipeline, and threading model. |
| [price-domain.md](./price-domain.md) | The logical `f64` price domain, render seam, precision guarantees, and prohibited casts. |
| [charting-engine.md](./charting-engine.md) | What the engine owns, renderer selection, browser support expectations, and product boundaries. |
| [testing.md](./testing.md) | Unit tests, parity harness, benches, and manual browser smoke guidance. |
| [drawing-persistence.md](./drawing-persistence.md) | Snapshot wire format, versioning, migration contract, and restore guidance. |
| [indicator-runtime.md](./indicator-runtime.md) | Indicator IR, runtime limits, MTF plumbing, and planned Worker offload path. |
| [feature-matrix.md](./feature-matrix.md) | Verified implementation status by subsystem. |
| [performance.md](./performance.md) | Benchmark-backed performance notes, renderer tradeoffs, and known scaling thresholds. |
| [events.md](./events.md) | Event catalog, payload tables, lifecycle rules, and delivery guarantees. |
| [migration-notes.md](./migration-notes.md) | This migration pass: baselines, behavior changes, artifact deltas, and deferred items. |
| [CONTRIBUTING.md](./CONTRIBUTING.md) | Build rules, style rules, doc invariants, and extension recipes. |
