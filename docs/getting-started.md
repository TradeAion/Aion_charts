# Getting Started

## Install

```bash
npm install aion_charts-wasm
```

For local development in this repository:

```bash
cargo check
cargo check --target wasm32-unknown-unknown -p aion_charts-wasm
cargo test
wasm-pack build wasm --target web --release
```

## Create A Chart

```html
<div id="chart" style="width: 100%; height: 420px;"></div>

<script type="module">
  import init, { Aion_charts } from './pkg/aion_charts_wasm.js';

  await init();

  const chart = await Aion_charts.create_chart('chart', {
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

  chart.on('visibleRangeChange', ({ startBar, endBar }) => {
    console.log('visible range', startBar, endBar);
  });
</script>
```

### Why `Float64Array`?

Logical prices are stored and processed as `f64` across the engine, persistence layer, studies, and WASM boundary. That preserves values such as `103842.5712345` and `0.00000012345678` exactly through the Aion_charts data path. Only the final renderer projection converts into single-precision GPU-friendly attributes.

## Initialization Rules

- Call `init()` before creating any charts.
- Serve the `.wasm` file with `Content-Type: application/wasm`.
- Give the container an explicit height before `create_chart(...)`.
- Call `dispose()` when removing the chart.

## Manual Rendering

`autoRender` defaults to `true`. When you need explicit frame control:

```ts
const chart = await Aion_charts.create_chart(host, { autoRender: false });
chart.render();
```

Auto-render is invalidation-driven. Aion_charts schedules a one-shot RAF when state changes; it does not spin a permanent RAF loop.

## Common Next Steps

- [API Reference](./api-reference.md)
- [Events](./events.md)
- [Framework Guide](./framework-guide.md)
- [Drawing Persistence](./drawing-persistence.md)
