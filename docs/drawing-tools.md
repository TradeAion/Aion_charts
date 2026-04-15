# Drawing Tools

AxiusCharts includes interactive drawing tools for technical analysis annotation.

---

## Available Tools

| Tool | Key | Anchors | Description |
|---|---|---|---|
| `trend_line` | — | 2 | Line segment between two points |
| `horizontal_line` | — | 1 | Full-width line at a price level |
| `vertical_line` | — | 1 | Full-height line at a bar index |
| `ray` | — | 2 | Line from point A through point B, extending to pane edge |
| `rectangle` | — | 2 | Filled rectangle between two corners |
| `fibonacci` | — | 2 | Fibonacci retracement levels between two price points |
| `scale` | — | 2 | Price range measurement (shows %, bars, price delta) |
| `brush` | — | drag | Freehand polyline drawn by dragging |

---

## Usage

### Activate a Tool

```js
chart.set_drawing_tool('trend_line');
```

### Creation Flow

1. Call `set_drawing_tool(toolName)` to activate
2. Click on the chart to place anchor points
3. For 2-anchor tools: first click places anchor[0], mouse move shows preview, second click places anchor[1] and completes
4. For 1-anchor tools (horizontal_line, vertical_line): single click completes
5. For brush: click-and-drag records the polyline, release completes
6. After completion, the tool deactivates and the drawing is selected

### Cancel / Delete

```js
chart.cancel_drawing();            // Cancel in-progress creation (Escape)
chart.remove_selected_drawing();   // Delete selected drawing (Delete/Backspace)
chart.clear_drawings();            // Remove all drawings
chart.remove_all_scale_drawings(); // Remove only scale/measurement drawings
```

### Persist / Restore Drawings

```js
// Save
const snapshot = chart.export_drawings();
localStorage.setItem('my-chart-drawings', snapshot);

// Restore
const saved = localStorage.getItem('my-chart-drawings');
if (saved) {
  chart.import_drawings(saved);
}
```

For production persistence patterns (full chart state, autosave, keying, pane mapping, and server storage), see [Persistent State Guide](./persistent.md).

### Keyboard Shortcuts

| Key | Action |
|---|---|
| `Delete` / `Backspace` | Remove selected drawing |
| `Escape` | Cancel creation, deselect all |

Forward keyboard events to the chart:

```js
document.addEventListener('keydown', (e) => {
  chart.on_key_down(e.key, e.ctrlKey, e.shiftKey, e.altKey);
});
```

---

## Events

```js
chart.on('drawingCreated', (e) => {
  console.log('New drawing:', e.id, e.tool);
});

chart.on('drawingSelected', (e) => {
  console.log('Selected drawing:', e.id);
});
```

---

## Interaction

- **Click** a drawing to select it (shows anchor handles)
- **Drag** the body to move the entire drawing
- **Drag** an anchor handle to reposition that point
- **Click** empty space to deselect
