# Persistent State Guide

How to persist RayCore chart state without hardcoding: drawings, drawing styles, chart styles/theme, viewport, and pane layout.

---

## Recommended API

Use full-state persistence by default:

- `chart.export_persistence_state(layoutId?)`
- `chart.import_persistence_state(json)`

This snapshot already includes:

- Drawing geometry and per-drawing style
- Chart style/theme options (layout, grid, crosshair, last price line, separators, fonts, colors, chart type)
- Symbol and interval
- Viewport (visible range, price range/lock, scale mode/margins, auto-scroll)
- Indicator sub-pane layout and pane viewport state

Use drawings-only persistence only when intentionally needed:

- `chart.export_drawings()`
- `chart.import_drawings(json)`

---

## Restore Order

1. Create chart.
2. Load OHLCV data.
3. Recreate studies you support.
4. Call `import_persistence_state(snapshot)`.

If a study from the saved snapshot does not exist at restore time, its pane is skipped.

---

## Client Integration (No Hardcoded IDs)

Use an app-defined `layoutId` and key by user/workspace/context.

```ts
const layoutId = activeLayout.id;
const storageKey = `raycore:persistence:${userId}:${workspaceId}:${symbol}:${interval}:${layoutId}`;

function saveLocal() {
  localStorage.setItem(storageKey, chart.export_persistence_state(layoutId));
}

function restoreLocal() {
  const raw = localStorage.getItem(storageKey);
  if (!raw) return;
  chart.import_persistence_state(raw);
}
```

Why this works:

- Pane IDs are remapped internally during import, so runtime IDs do not need to be stable.
- Styles are included in the snapshot, so theme/layout/font changes restore as saved.

---

## Autosave Pattern

Persist with debounce to avoid write storms.

```ts
const save = debounce(() => {
  persistLayout({
    userId,
    workspaceId,
    symbol,
    interval,
    layoutId,
    snapshot: chart.export_persistence_state(layoutId),
  });
}, 150);

chart.on('drawingCreated', save);
document.addEventListener('pointerup', save, { passive: true });
window.addEventListener('pagehide', () => {
  persistLayout({
    userId,
    workspaceId,
    symbol,
    interval,
    layoutId,
    snapshot: chart.export_persistence_state(layoutId),
  });
});
```

Also trigger save after your style UI applies options (theme/color/font/line settings).

---

## Postgres Storage Model

Use JSONB so you can store the full snapshot exactly as produced by RayCore.

```sql
create table chart_layouts (
  id uuid primary key,
  user_id uuid not null,
  workspace_id uuid not null,
  symbol text not null,
  interval text not null,
  layout_name text not null,
  snapshot jsonb not null,
  snapshot_version int not null default 1,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (user_id, workspace_id, symbol, interval, layout_name)
);

create index chart_layouts_lookup_idx
  on chart_layouts (user_id, workspace_id, symbol, interval);

create index chart_layouts_snapshot_gin_idx
  on chart_layouts using gin (snapshot);
```

### Optional: user style presets

If your product allows users to save reusable style-only presets (independent of drawings/layout), keep a second table:

```sql
create table chart_style_presets (
  id uuid primary key,
  user_id uuid not null,
  workspace_id uuid not null,
  preset_name text not null,
  options jsonb not null,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (user_id, workspace_id, preset_name)
);
```

`options` should store an `apply_options(...)`-compatible object (layout/grid/crosshair/priceScale/lastPriceLine/font/colors/chartType).

---

## Backend API Shape (Example)

```http
PUT /api/chart-layouts/:layoutId
GET /api/chart-layouts/:layoutId?symbol=BTCUSD&interval=1m
PUT /api/chart-style-presets/:presetId
GET /api/chart-style-presets
```

Request body for layout save:

```json
{
  "userId": "uuid",
  "workspaceId": "uuid",
  "symbol": "BTCUSD",
  "interval": "1m",
  "layoutId": "uuid",
  "layoutName": "Main",
  "snapshot": "{...string from export_persistence_state...}"
}
```

On restore:

1. Fetch layout by user/workspace/layout/symbol/interval.
2. Pass `snapshot` directly to `chart.import_persistence_state(snapshot)`.

No manual remapping logic is needed in the app layer.

---

## Versioning and Validation

- Reject malformed JSON at API boundary.
- Store `snapshot_version` separately for query/migration operations.
- Keep snapshot payload immutable; avoid patching nested fields server-side.
- If you need migrations later, run explicit migration jobs by `snapshot_version`.

---

## Multi-Layout Support

A user can have multiple layouts per symbol/interval (`Main`, `Scalping`, `Swing`, etc.).

Recommended selector identity:

- `userId`
- `workspaceId`
- `symbol`
- `interval`
- `layoutId` (or unique `layoutName`)

This gives full flexibility for drawing persistence + style persistence + layout persistence with zero hardcoded values in client code.
