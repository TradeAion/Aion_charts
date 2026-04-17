# Drawing Tools

AxiusCharts includes interactive drawing tools for chart annotation and measurement.

## Available Tools

| Tool | Anchors | Purpose |
| --- | --- | --- |
| `trend_line` | 2 | Segment between two logical points |
| `horizontal_line` | 1 | Full-width price level |
| `vertical_line` | 1 | Full-height time marker |
| `ray` | 2 | Semi-infinite line from point A through point B |
| `rectangle` | 2 | Filled region between two corners |
| `fibonacci` | 2 | Retracement ladder between two anchor prices |
| `scale` | 2 | Range measurement |
| `brush` | drag | Freehand polyline |

## Workflow

1. Call `set_drawing_tool(toolName)`
2. Place the required anchors
3. Drag existing drawings or anchors to edit them
4. Use `cancel_drawing()`, `remove_selected_drawing()`, or `clear_drawings()` as needed

## Keyboard Forwarding

```ts
document.addEventListener('keydown', (e) => {
  chart.on_key_down(e.key, e.ctrlKey, e.shiftKey, e.altKey);
});
```

## Persistence

```ts
const snapshot = chart.export_drawings();
chart.import_drawings(snapshot);
```

For full layout persistence, snapshot versioning, and migration rules, see [drawing-persistence.md](./drawing-persistence.md).

## Precision Contract

Drawing anchors store logical bar indices and `f64` prices. Persistence never truncates those anchor prices to render precision.
