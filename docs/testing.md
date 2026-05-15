# Testing

Aion_charts uses three layers of verification: Rust unit tests, renderer parity scaffolding, and manual browser smoke checks.

## Unit And Doc Tests

```bash
cargo test
```

This covers:

- core data structures
- viewport math
- event emission
- drawing persistence
- studies and indicators
- doc tests embedded in Rust source

## Backend Parity Harness

```bash
cargo test --features parity-tests
```

This enables the native-only structural parity scaffold:

- WebGPU renders fixtures into an offscreen surface
- the mock Canvas2D path records structural draw calls
- fixture results are compared for geometry count, positions, and colors
- a report is written to `target/backend-parity-report.md`

The harness is intentionally structural, not pixel-based. A future headless-browser pass can add pixel diffing without changing the default test suite.

## Benchmarks

```bash
cargo bench --bench core_benchmarks
```

Use this when touching:

- `BarArray`
- viewport transforms
- autoscale logic
- renderer geometry generation

See [performance.md](./performance.md) for the captured baseline and post-migration results.

## Browser Smoke Pass

Manual smoke workflow:

1. Build with `wasm-pack build wasm --target web --release`
2. Run `python serve.py`
3. Open `http://localhost:8080/demo/`
4. Verify page load, pointer drag, wheel zoom, drawing creation, execution mark toggle, and resize behavior
5. Confirm no new console or runtime errors

## Future Pixel-Diff Plan

Future work:

- run a real headless browser harness
- capture deterministic screenshots per fixture
- compare backend output with pixel tolerances
- keep the structural parity harness as the fast inner loop
