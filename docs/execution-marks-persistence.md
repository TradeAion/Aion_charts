# Execution Marks Persistence

Execution marks serialize through a versioned JSON snapshot so future schema changes can migrate forward instead of failing silently.

## Snapshot Shape

Current wire format:

```json
{
  "version": 1,
  "marks": [
    {
      "id": "exec-1",
      "timestamp_ms": 1700000000000,
      "price": 45000.5,
      "quantity": 0.5,
      "side": "buy",
      "role": "entry",
      "order_type": "market",
      "realized_pnl": null,
      "label": null,
      "color": null,
      "group_id": "trade-1"
    }
  ]
}
```

## Field Contract

Required fields per mark:

- `id: string`
- `timestamp_ms: u64 / number`
- `price: f64 / number`
- `quantity: f64 / number`
- `side: "buy" | "sell"`
- `role: "entry" | "scale_in" | "scale_out" | "exit"`

Optional fields per mark:

- `order_type: string | null`
- `realized_pnl: number | null`
- `label: string | null`
- `color: [number, number, number, number] | null`
- `group_id: string | null`

Import compatibility aliases:

- `timestampMs`
- `orderType`
- `realizedPnl`
- `groupId`

Legacy bare-array JSON remains accepted on import. It is interpreted as version `1`.

## Version Policy

- `EXECUTION_MARKS_SNAPSHOT_VERSION` is currently `1`
- `get_execution_marks_json()` always emits the wrapped object form
- `set_execution_marks_json()` accepts either:
  - wrapped `{ version, marks }`
  - legacy bare `[...]`
- Versions greater than the current version are rejected with a clear error

## Migration Seam

The core migration hook is:

```rust
pub fn migrate_execution_marks_snapshot(
    value: serde_json::Value,
    from_version: u32,
) -> Result<serde_json::Value, String>
```

Current behavior:

- `from_version == 1` returns the payload unchanged
- Any other version is rejected until an explicit migration step is added

When a future schema change lands:

1. Bump `EXECUTION_MARKS_SNAPSHOT_VERSION`
2. Add `migrate_v1_to_v2(...)`, `migrate_v2_to_v3(...)`, and so on
3. Chain the functions inside `migrate_execution_marks_snapshot(...)`
4. Keep the old input format accepted until the chain can upgrade it

Skeleton:

```rust
pub fn migrate_execution_marks_snapshot(
    mut value: serde_json::Value,
    from_version: u32,
) -> Result<serde_json::Value, String> {
    match from_version {
        1 => {
            value = migrate_v1_to_v2(value)?;
            Ok(value)
        }
        other => Err(format!("unsupported execution marks snapshot migration from version {}", other)),
    }
}
```

## Precision Contract

Execution-mark persistence stays in the logical `f64` domain:

- prices stay double precision
- realized P&L stays double precision
- no `f32` downcast occurs in serialization

That means the snapshot round-trips the same logical-domain values the engine uses for render planning, hit-testing, and event payloads.
