# AxiusCharts

**High-performance WebGPU/Canvas2D charting engine built in Rust + WASM.**

Created by [AxiusCharts](https://github.com/Axiusflow/Axius_charts/)

---

## Quick Start

```bash
npm install axiuscharts-wasm
```

```js
import init, { AxiusCharts } from 'axiuscharts-wasm';

await init();

const chartHost = document.getElementById('container');
if (!chartHost) throw new Error("Missing container element with id 'container'");

const chart = await AxiusCharts.create_chart(chartHost, {
  renderer: 'webgpu',
  autoRender: true,
  theme: 'dark',
});

chart.set_data_arrays(opens, highs, lows, closes, volumes, timestamps);
```

---

## Features

- **GPU-accelerated rendering** via WebGPU with Canvas2D fallback
- **6 chart types**: Candlestick, OHLC, Line, Area, Heikin-Ashi, Baseline
- **8 drawing tools**: Trend Line, Horizontal Line, Vertical Line, Ray, Rectangle, Fibonacci, Scale, Brush
- **8 built-in studies**: SMA, EMA, RSI, MACD, Bollinger Bands, Stochastic, ATR, VWAP
- **Overlay series**: Line, Area, Histogram, Bar, Baseline
- **Series markers**: Arrows, circles, squares at bar indices
- **Execution marks**: Trade visualization (entry/exit/scale-in/scale-out) with timestamp-based placement
- **Price lines**: Horizontal price level annotations
- **Multi-pane**: Indicator sub-panes, synchronized chart groups, split workspaces
- **Typed event system**: crosshairMove, click, visibleRangeChange, drawing events, execution mark events
- **Theme system**: Dark/light presets, CSS variable integration
- **Framework-agnostic**: Works with React, Vue, Svelte, vanilla JS

---

## Documentation

| Doc | Description |
|---|---|
| [Getting Started](./docs/getting-started.md) | Installation, quick start, WASM init |
| [API Reference](./docs/api-reference.md) | Complete method documentation |
| [Framework Guide](./docs/framework-guide.md) | React, Vue, Svelte, bundler config |
| [Theming](./docs/theming.md) | Dark/light, custom colors, CSS variables |
| [Drawing Tools](./docs/drawing-tools.md) | Interactive drawing tools |

---

## Build from Source

### Prerequisites

- Rust 1.83+ with `wasm32-unknown-unknown` target
- [wasm-pack](https://rustwasm.github.io/wasm-pack/)

### Compile

```bash
wasm-pack build --target web --out-dir pkg wasm
```

### Run Demo

```bash
python serve.py
# Open http://localhost:8080/demo/
```

Use the `Exec` button in the demo toolbar to toggle sample execution marks on the time-based chart modes.

---

## Performance

- 10,000+ candles in a single instanced GPU draw call
- Sub-millisecond frame times on integrated GPUs
- Zero GC pressure (no JS objects created per frame)
- Automatic RAF rendering when enabled, with manual `render()` available

---

## License

Proprietary. All rights reserved. See [LICENSE](./LICENSE).

*Built by [AxiusCharts](https://github.com/Axiusflow/Axius_charts/)*
