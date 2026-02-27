# Theming

RayCore supports dark and light themes out of the box, plus custom color overrides via the theme API.

---

## Built-in Presets

```js
// Dark theme (default)
const chart = await RayCore.create_chart(container, { theme: 'dark' });

// Light theme
const chart = await RayCore.create_chart(container, { theme: 'light' });
```

Switch at runtime:

```js
chart.apply_options({ theme: 'light' });
```

---

## CSS Variables

RayCore injects CSS custom properties onto the chart container, enabling seamless integration with your app's styling:

```js
const vars = chart.get_css_variables();
// Returns an object with all CSS variable key-value pairs
```

Injected variables include:

| Variable | Description |
|---|---|
| `--raycore-bg` | Chart background color |
| `--raycore-text` | Axis text color |
| `--raycore-border` | Axis border color |
| `--raycore-grid` | Grid line color |
| `--raycore-crosshair` | Crosshair line color |
| `--raycore-bullish` | Bullish candle color |
| `--raycore-bearish` | Bearish candle color |

---

## Candle Colors

```js
// Set bullish (up) candle fill and wick colors
chart.set_bullish_color(r, g, b, a, wick_r, wick_g, wick_b, wick_a);

// Set bearish (down) candle fill and wick colors
chart.set_bearish_color(r, g, b, a, wick_r, wick_g, wick_b, wick_a);

// Set volume bar colors (up, down)
chart.set_volume_colors(up_r, up_g, up_b, up_a, down_r, down_g, down_b, down_a);
```

All color values are RGBA floats in the 0.0-1.0 range.

---

## Crosshair Styling

```js
// Line color (per-target)
chart.set_crosshair_line_color('both', r, g, b, a);  // 'vert', 'horz', or 'both'

// Line style
chart.set_crosshair_line_style('both', 'dashed');  // 'solid', 'dotted', 'dashed', 'large_dashed', 'sparse_dotted'

// Line width (CSS px)
chart.set_crosshair_line_width('both', 1);

// Visibility
chart.set_crosshair_line_visible('vert', true);
chart.set_crosshair_label_visible('horz', false);

// Label background color
chart.set_crosshair_line_label_bg_color('both', r, g, b, a);
```

---

## Font

```js
chart.set_font_family('Inter, system-ui, sans-serif');
chart.set_font_size(12);
```
