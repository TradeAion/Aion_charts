# Drawing Persistence

Aion_charts exposes both drawings-only and full-layout persistence. Drawings are versioned through `DRAWINGS_SNAPSHOT_VERSION`, and snapshot import now routes through a version-aware migration entry point.

## Wire Format

The drawings snapshot format is JSON:

```json
{
  "version": 1,
  "drawings": [
    {
      "id": 7,
      "tool": "trend_line",
      "style": {
        "color": [0.2, 0.7, 0.9, 1.0],
        "line_width": 2.0,
        "fill_color": null,
        "dash": null,
        "font_size": 12.0
      },
      "anchors": [
        {
          "point": {
            "bar_index": 10.0,
            "price": 100.25,
            "timestamp": 1700000000000
          },
          "hit_radius": 5.0
        }
      ],
      "points": []
    }
  ]
}
```

Important properties:

- `version` is required for migration decisions
- `bar_index` is logical chart space
- `price` is stored as logical-domain `f64`
- `timestamp` is optional and preserved when present

## Public Contracts

- `DRAWINGS_SNAPSHOT_VERSION` is a public wire-format constant
- `migrate_snapshot(payload)` loads a JSON value into the current `DrawingSnapshot`
- `export_drawings()` / `import_drawings(...)` handle drawings only
- `export_persistence_state()` / `import_persistence_state(...)` include drawings plus chart layout state

## Migration API

Current behavior:

- If `version == DRAWINGS_SNAPSHOT_VERSION`, deserialize directly
- If `version < DRAWINGS_SNAPSHOT_VERSION`, apply each step migration in order
- If `version > DRAWINGS_SNAPSHOT_VERSION`, return `DrawingsMigrationError::Incompatible`
- Missing or invalid versions return `DrawingsMigrationError::UnknownVersion`

When adding a new version:

1. Bump `DRAWINGS_SNAPSHOT_VERSION`
2. Add `migrate_vN_to_vN_plus_1(...)`
3. Wire that step into the migration chain
4. Add round-trip and compatibility tests

## Round-Trip Guarantees

- Drawing anchor prices stay in `f64`
- Exported snapshots do not pass through GPU precision
- Import is validated before replace, so malformed payloads do not partially wipe existing state

## Recommended Restore Order

1. Create the chart
2. Load market data
3. Recreate studies or panes required by your layout
4. Call `import_persistence_state(...)` or `import_drawings(...)`

This keeps bar-index and timestamp mapping stable before drawings are restored.
