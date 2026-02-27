# Getting Started

## Installation

### npm / yarn / pnpm

```bash
npm install raycore-wasm
```

### Manual (from GitHub release)

Download the latest `raycore-wasm.tar.gz` from [Releases](https://github.com/devrajsingh15/raycore/releases), extract, and place the files in your project.

### CDN (ESM)

```html
<script type="module">
  import init, { RayCore } from 'https://unpkg.com/raycore-wasm/raycore_wasm.js';
</script>
```

---

## Quick Start

```html
<div id="chart" style="width: 100%; height: 400px;"></div>

<script type="module">
  import init, { RayCore } from './pkg/raycore_wasm.js';

  await init();

  const chart = await RayCore.create_chart('chart', {
    renderer: 'auto',       // 'webgpu', 'canvas2d', or 'auto'
    autoRender: true,        // starts RAF loop automatically
    theme: 'dark',           // 'dark' or 'light'
    symbol: 'BTCUSD',
    interval: '1m',
  });

  // Load OHLCV data (parallel typed arrays)
  chart.set_data_arrays(
    new Float32Array(opens),
    new Float32Array(highs),
    new Float32Array(lows),
    new Float32Array(closes),
    new Float32Array(volumes),
    new BigUint64Array(timestamps),  // millisecond unix timestamps
  );
</script>
```

---

## WASM Initialization

RayCore is a WebAssembly module. Before using any API, you must initialize the WASM runtime:

```js
import init, { RayCore } from 'raycore-wasm';

// init() fetches and compiles the .wasm file.
// It is idempotent — safe to call multiple times.
await init();
```

The `.wasm` file must be served with `Content-Type: application/wasm`. Most dev servers handle this automatically. For production bundlers, see the [Framework Guide](./framework-guide.md).

---

## Container Requirements

The chart container **must** have explicit dimensions (width and height). RayCore uses a `ResizeObserver` to track size changes, but the container itself must have non-zero dimensions before chart creation:

```css
/* Good — explicit dimensions */
#chart { width: 100%; height: 400px; }

/* Bad — no height, chart will be 0px tall */
#chart { width: 100%; }
```

---

## Lifecycle

```js
// Create
const chart = await RayCore.create_chart(container, options);

// Use
chart.set_data_arrays(...);
chart.set_chart_type('candlestick');
chart.on('crosshairMove', (e) => console.log(e));

// Cleanup
chart.dispose();
```

Always call `dispose()` when removing the chart from the DOM to detach event listeners and free WASM memory.

---

## Next Steps

- [API Reference](./api-reference.md) — Complete method documentation
- [Framework Guide](./framework-guide.md) — React, Vue, Svelte integration
- [Theming](./theming.md) — Dark/light themes, custom colors, CSS variables
- [Drawing Tools](./drawing-tools.md) — Interactive drawing tools
