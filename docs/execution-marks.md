# Execution Marks

Execution marks are first-class trade annotations for fills, exits, scale-ins, and scale-outs. They resolve through the shared `TimeScaleIndex`, render on the main price pane, participate in hover/click events, and serialize through a versioned JSON snapshot.

## Core Model

Each execution mark carries:

- Required: `id`, `timestamp_ms`, `price`, `quantity`, `side`, `role`
- Optional: `order_type`, `realized_pnl`, `label`, `color`, `group_id`

`price` and `realized_pnl` use the same logical `number` / Rust `f64` domain as bars, studies, and drawings.

## Basic Usage

```ts
chart.add_execution_mark(
  'exec-1',
  1_700_000_000_000,
  45_000.50,
  0.5,
  'buy',
  'entry',
);

chart.add_execution_mark_full(
  'exec-2',
  1_700_000_060_000,
  45_250.00,
  0.5,
  'sell',
  'exit',
  'limit',
  '',
  'trade-1',
  0, 0, 0, 0,
  150.0,
);
```

## Label Modes

The engine owns one chart-wide execution label mode. Per-mark `label` still overrides the mode.

- `side`: `BUY` / `SELL`
- `role`: `ENTRY` / `SCALE IN` / `SCALE OUT` / `EXIT`
- `side_and_role`: `BUY · ENTRY`

```ts
chart.set_execution_label_mode('side_and_role');
console.log(chart.get_execution_label_mode()); // "side_and_role"
```

## P&L Visibility

Realized P&L renders only for `scale_out` and `exit` marks when:

- the mark has `realized_pnl`
- text rendering is enabled
- chart-level P&L visibility is enabled

Formatting rules:

- Positive: `+$123.45`
- Negative: `-$67.89`
- Zero: `$0.00`
- For sub-dollar price references, the formatter keeps 4 decimals

```ts
chart.set_execution_pnl_visible(true);
console.log(chart.get_execution_pnl_visible()); // true
```

## Automatic Clustering

Dense same-side fills are clustered at render time. Storage remains unchanged; only the overlay collapses them.

- Clustering key: same side and projected X distance under the current threshold
- Cluster anchor: quantity-weighted VWAP position
- Cluster badge: `×N`
- Hover on a cluster reveals all member chevrons at their exact prices
- Click on a cluster still selects the leader mark and also emits `executionClusterClick`

Set the threshold to `0` to disable clustering effectively.

```ts
chart.set_execution_cluster_threshold_px(14);
chart.set_execution_cluster_threshold_px(0); // disable clustering
```

If the frontend needs the expanded member list for a visible cluster:

```ts
const members = chart.expand_execution_cluster('exec-1');
console.log(members);
```

## Bulk Load And JSON

Flat-array bulk path:

```ts
chart.set_execution_marks(
  ['exec-1', 'exec-2'],
  new Float64Array([
    1_700_000_000_000, 45_000.5, 0.5, 0, 0,
    1_700_000_060_000, 45_250.0, 0.5, 1, 3,
  ]),
);
```

JSON snapshot path:

```ts
chart.set_execution_marks_json(JSON.stringify({
  version: 1,
  marks: [
    {
      id: 'exec-1',
      timestamp_ms: 1_700_000_000_000,
      price: 45_000.5,
      quantity: 0.5,
      side: 'buy',
      role: 'entry',
      group_id: 'trade-1',
    },
    {
      id: 'exec-2',
      timestamp_ms: 1_700_000_060_000,
      price: 45_250.0,
      quantity: 0.5,
      side: 'sell',
      role: 'exit',
      realized_pnl: 150.0,
      group_id: 'trade-1',
    },
  ],
}));

const snapshot = JSON.parse(chart.get_execution_marks_json());
console.log(snapshot.version); // 1
```

Legacy bare-array JSON is still accepted on import for backward compatibility.

## Selection And Events

Execution marks participate in:

- `executionMarkHover`
- `executionMarkClick`
- `executionClusterClick`

```ts
chart.on('executionClusterClick', ({ leaderId, memberIds }) => {
  console.log('cluster', leaderId, memberIds);
});

chart.set_selected_execution_mark('exec-1');
console.log(chart.get_selected_execution_mark()); // "exec-1"
chart.clear_selected_execution_mark();
```

Grouped trades render only per-fill chevrons when selected or hovered through clustering. No connector lines are emitted between grouped fills.

## Persistence

See [execution-marks-persistence.md](./execution-marks-persistence.md) for the wire format and migration contract.
