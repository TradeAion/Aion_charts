# @aion/charts

A trading-chart engine in **Rust + WebGPU + WASM**, pixel-faithful to
[lightweight-charts](https://github.com/tradingview/lightweight-charts) v5, with a plain
TypeScript API. Canvas2D fallback with automatic device-loss failover included.

## Install

[Bun](https://bun.sh) is the primary, recommended package manager:

```sh
bun add @aion/charts
```

npm works identically:

```sh
npm install @aion/charts
```

No Rust toolchain or native build step is needed — the package ships prebuilt JS + WASM.

## Quick start

```ts
import { create_chart } from "@aion/charts";

// Async: WebGPU backend acquisition (the one deliberate divergence from LWC's sync createChart).
const chart = await create_chart(document.getElementById("chart"), {
  layout: { background: { type: "solid", color: "#ffffff" }, textColor: "#191919" },
});

const series = chart.add_series("candlestick", { up_color: "#26a69a", down_color: "#ef5350" });
series.set_data([
  { time: "2026-01-01", open: 100, high: 104, low: 99, close: 103 },
  { time: "2026-01-02", open: 103, high: 106, low: 102, close: 105 },
]);

chart.time_scale().fit_content();
```

API semantics mirror lightweight-charts v5 (options, series handles, time/price scale handles,
events), with snake_case naming. See the
[repository](https://github.com/TradeAion/Aion_charts) for the full docs.

## Bundler notes

The package is ESM-only and ships two artifacts side by side in `dist/`: `index.js` and
`aion_wasm_bg.wasm`. The wasm is fetched relative to the bundle
(`new URL("aion_wasm_bg.wasm", import.meta.url)`).

- **webpack 5 / Next.js / Vite production builds**: works out of the box (the wasm is emitted as
  an asset).
- **Vite dev server**: the dep optimizer rebundles `node_modules`, breaking the co-location.
  Either exclude the package from pre-bundling:

  ```ts
  // vite.config.ts
  export default { optimizeDeps: { exclude: ["@aion/charts"] } };
  ```

  or point the engine at an explicit wasm URL:

  ```ts
  import { create_chart, init_wasm } from "@aion/charts";
  import wasm_url from "@aion/charts/dist/aion_wasm_bg.wasm?url";

  await init_wasm(wasm_url); // call once, before the first create_chart
  ```
- **Plain static hosting / `<script type="module">`**: works as-is.

## Runtime requirements

Browser only (DOM + `fetch` of the wasm asset; WebGPU optional — falls back to Canvas2D).
Importing the module in Node/SSR is safe (side-effect-free); calling `create_chart` requires a
browser environment.

## License

[MIT](../../LICENSE)
