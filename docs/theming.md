# Theming

AxiusCharts supports built-in dark and light presets plus partial custom theme objects passed through `create_chart(...)` or `apply_options(...)`.

## Presets

```ts
const chart = await AxiusCharts.create_chart(host, { theme: 'dark' });
chart.apply_options({ theme: 'light' });
```

## CSS Variables

```ts
const vars = chart.get_css_variables();
```

Common emitted variables:

- `--axiuscharts-bg`
- `--axiuscharts-text`
- `--axiuscharts-border`
- `--axiuscharts-grid`
- `--axiuscharts-crosshair`
- `--axiuscharts-bullish`
- `--axiuscharts-bearish`

## Custom Theme Objects

`apply_options({ theme: { ... } })` accepts nested color, crosshair, typography, layout, and series overrides. Omitted fields fall back to the active preset.

## Targeted Runtime Overrides

Legacy targeted setters remain available for narrow runtime adjustments:

- `set_crosshair_line_color`
- `set_crosshair_line_style`
- `set_crosshair_line_width`
- `set_font_family`
- `set_font_size`
- `set_bullish_color`
- `set_bearish_color`
- `set_volume_colors`

Prefer `apply_options({ theme: ... })` for app-level theming and use the targeted setters only when you need direct imperative control.
