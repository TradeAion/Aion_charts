# Drawing Tools

Aion_charts includes interactive drawing tools for chart annotation and measurement.

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
4. Use `complete_drawing()`, `cancel_drawing()`, `deselect_drawings()`, `remove_selected_drawing()`, or `clear_drawings()` as needed

## Shortcut Ownership

Aion_charts does not own keyboard shortcuts. The engine exposes chart and drawing
commands; the platform embedding the engine decides whether shortcuts exist and
which keys invoke those commands.

Do not add tool shortcut mapping to the engine. Add shortcut mapping in the
application, shell, or platform layer that hosts the chart. This keeps shortcuts
configurable for product needs, user preferences, operating systems, locales,
permissions, and focused UI state.

Example platform mapping:

```ts
document.addEventListener('keydown', (e) => {
  if (isEditingFormControl(e.target)) return;

  const toolShortcuts = {
    t: 'trend_line',
    r: 'rectangle',
    f: 'fibonacci',
    m: 'scale',
  };

  const tool = toolShortcuts[e.key.toLowerCase()];
  if (tool && !e.ctrlKey && !e.metaKey && !e.altKey) {
    chart.set_drawing_tool(tool);
    e.preventDefault();
    return;
  }

  if (e.key === 'Enter') {
    if (chart.complete_drawing()) e.preventDefault();
  } else if (e.key === 'Escape') {
    chart.cancel_drawing();
    chart.deselect_drawings();
    e.preventDefault();
  } else if (e.key === 'Delete' || e.key === 'Backspace') {
    chart.remove_selected_drawing();
    e.preventDefault();
  }
});
```

This example is optional platform code, not required engine integration.

## Persistence

```ts
const snapshot = chart.export_drawings();
chart.import_drawings(snapshot);
```

For full layout persistence, snapshot versioning, and migration rules, see [drawing-persistence.md](./drawing-persistence.md).

## Precision Contract

Drawing anchors store logical bar indices and `f64` prices. Persistence never truncates those anchor prices to render precision.
