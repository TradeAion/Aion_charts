# Events

AxiusCharts emits typed chart events through `on(...)`, `off(...)`, and `once(...)`. Public JS payload fields use camelCase.

## Subscription Lifecycle

```ts
const onRange = ({ startBar, endBar }) => {
  console.log(startBar, endBar);
};

chart.on('visibleRangeChange', onRange);
chart.off('visibleRangeChange', onRange);
chart.once('click', ({ price }) => console.log(price));
```

- `on(name, fn)` registers a persistent callback
- `off(name, fn)` removes the exact same function reference
- `once(name, fn)` removes the callback after the first delivery
- `dispose()` drops listeners and stops future delivery

## Event Catalog

| Event | Payload |
| --- | --- |
| `crosshairMove` | `type`, `x`, `y`, `barIndex`, `price`, `timestamp` |
| `click` | `type`, `x`, `y`, `barIndex`, `price` |
| `visibleRangeChange` | `type`, `startBar`, `endBar` |
| `symbolChange` | `type`, `symbol` |
| `intervalChange` | `type`, `interval` |
| `chartTypeChange` | `type`, `chartType` |
| `priceScaleChange` | `type`, `mode` |
| `resize` | `type`, `width`, `height` |
| `rendererFallback` | `type`, `requested`, `active`, `reason` |
| `drawingCreated` | `type`, `id`, `tool` |
| `drawingSelected` | `type`, `id` |
| `executionClusterClick` | `type`, `leaderId`, `memberIds` |
| `executionMarkClick` | `type`, `id`, `timestampMs`, `price`, `side`, `role`, `quantity`, `groupId` |
| `executionMarkHover` | `type`, `id`, `timestampMs`, `price`, `side`, `role`, `quantity`, `groupId` |
| `error` | `type`, `message` |

## Delivery Guarantees

- `crosshairMove` is at most once per RAF frame.
- `visibleRangeChange` is at most once per RAF frame.
- `visibleRangeChange` is emitted during drag, wheel, pinch, reset, and kinetic glide when the logical visible range changes.
- No duplicate `visibleRangeChange` is emitted for a glide frame that leaves the range unchanged.
- `executionClusterClick` is emitted only when the clicked hit area represents two or more clustered execution marks.

## Public Naming Notes

Public event payload fields are camelCase even when the internal Rust event struct uses snake_case fields. Treat [`wasm/axiuscharts.d.ts`](../wasm/axiuscharts.d.ts) as the canonical payload contract.
