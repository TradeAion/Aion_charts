# AxiusCharts

AxiusCharts is a production charting engine built in Rust and shipped through WebAssembly. The core crate (`axiuscharts`) owns data storage, viewport math, geometry generation, render backends, drawings, indicators, and event delivery; the `axiuscharts-wasm` crate is the thin DOM and `wasm-bindgen` bridge.

The logical price domain is now `f64` end-to-end. Prices remain double precision in storage, math, persistence, indicators, and the JS/WASM boundary, then get projected into single-precision render space only at the renderer seam.

## Quick Start

```bash
npm install @axiusflow/axiuscharts-wasm
```

```ts
import init, { AxiusCharts } from '@axiusflow/axiuscharts-wasm';

await init();

const host = document.getElementById('chart');
if (!host) throw new Error('Missing chart container');

const chart = await AxiusCharts.create_chart(host, {
  renderer: 'auto',
  autoRender: true,
  theme: 'dark',
  symbol: 'BTCUSD',
  interval: '1m',
});

chart.set_data_arrays(
  new Float64Array(opens),
  new Float64Array(highs),
  new Float64Array(lows),
  new Float64Array(closes),
  new Float64Array(volumes),
  new BigUint64Array(timestamps),
);
```

## Guardrails

AxiusCharts can enforce engine-side commercial or operational caps without relying on UI-only checks.

```ts
const chart = await AxiusCharts.create_chart(host, {
  theme: 'dark',
  guardrails: {
    maxIndicatorPanes: 2,
    maxBarsPerLoad: 5000,
    allowedIntervals: ['1m', '5m', '1h'],
    lockInterval: false,
  },
});

chart.set_max_indicator_panes(3);
chart.set_max_bars_per_load(10000);
chart.set_allowed_intervals(['1m', '15m', '1h']);
chart.set_interval_change_locked(true);

const workspace = new ChartWorkspace('workspace-root');
workspace.set_max_panes(2);
```

Use `0` to disable a cap.

## Implemented Surface

- Main chart types: candlestick, OHLC, line, area, Heikin-Ashi, footprint
- Overlay series: line, area, histogram, bar
- Drawings: trend line, horizontal line, vertical line, ray, rectangle, Fibonacci, scale, brush
- Built-in studies: SMA, EMA, RSI, MACD, Bollinger Bands, Stochastic, ATR, VWAP
- Trade annotation: markers, draggable price lines, execution marks
- Multi-pane and groups: indicator panes, chart groups, synchronized ranges and crosshair state
- Renderers: WebGPU primary, Canvas2D fallback
- Events: `crosshairMove`, `click`, `visibleRangeChange`, drawing events, execution mark events, execution cluster events, lifecycle events

See [docs/feature-matrix.md](./docs/feature-matrix.md) for the verified matrix.

## Documentation

Start at [docs/README.md](./docs/README.md). The most important documents for this migration are:

- [docs/price-domain.md](./docs/price-domain.md)
- [docs/events.md](./docs/events.md)
- [docs/execution-marks.md](./docs/execution-marks.md)
- [docs/execution-marks-persistence.md](./docs/execution-marks-persistence.md)
- [docs/drawing-persistence.md](./docs/drawing-persistence.md)
- [docs/migration-notes.md](./docs/migration-notes.md)

## Build And Verify

```bash
cargo check
cargo check --target wasm32-unknown-unknown -p axiuscharts-wasm
cargo test
cargo clippy -- -D warnings
wasm-pack build wasm --target web --release
```

## Package Releases

This repository publishes the charting engine as the private GitHub Package `@axiusflow/axiuscharts-wasm`.

To release chart changes, bump `version` in `package.json` and push to `main`. The publish workflow builds the WASM package and publishes the new version to GitHub Packages. If the version already exists, the workflow skips publishing instead of overwriting it.

Apps should consume the chart engine by package version. Update the app dependency only when the app should adopt a new chart engine release.

Parity harness:

```bash
cargo test --features parity-tests
```

## Demo

```bash
python serve.py
```

Open `http://localhost:8080/demo/`.

## License

Proprietary. All rights reserved. See [LICENSE](./LICENSE).
