# Contributing

## Required Verification

Run these before handing off Rust changes:

```bash
cargo check
cargo check --target wasm32-unknown-unknown -p axiuscharts-wasm
cargo test
cargo clippy -- -D warnings
wasm-pack build wasm --target web --release
```

Optional but expected when renderer behavior changes:

```bash
cargo test --features parity-tests
```

## Style Rules

- Keep `rustfmt`-compatible formatting.
- Treat clippy warnings as errors.
- Do not change the public API shape unless the migration or issue explicitly allows it.
- Preserve the RAF self-reference slot pattern and the invalidation-driven one-shot render model.

## Documentation Invariants

Keep these in sync:

- `wasm/axiuscharts.d.ts`
- `docs/api-reference.md`
- `docs/events.md`
- `docs/feature-matrix.md`
- `README.md`

## Adding A New Event

1. Add the event variant in `src/core/events.rs`
2. Emit it from the engine path that owns the state change
3. Bridge it through the WASM emitter
4. Update `wasm/axiuscharts.d.ts`
5. Document it in `docs/events.md`

## Adding A New Drawing Tool

1. Define the tool and its anchors in `src/core/drawings`
2. Add rendering and hit-testing support
3. Ensure persistence serialization works
4. Update `docs/drawing-tools.md` and `docs/drawing-persistence.md`

## Adding A New Series Type

1. Add the series storage and validation types under `src/core/series`
2. Feed it into renderer geometry generation
3. Expose the WASM bridge methods and `.d.ts` signatures
4. Update `docs/feature-matrix.md` and `docs/api-reference.md`

## Adding A New Built-In Study

1. Implement the calculator under `src/core/studies/built_in`
2. Register it in the study manager
3. Expose parameter and output handling through WASM if needed
4. Add tests and update docs

## Adding A New Snapshot Version

1. Bump `DRAWINGS_SNAPSHOT_VERSION`
2. Add a `vN -> vN+1` migration step in `src/core/drawings/persistence.rs`
3. Chain it through `migrate_snapshot(...)`
4. Add round-trip, backward-compat, and future-version tests
5. Update `docs/drawing-persistence.md` and `docs/migration-notes.md`
