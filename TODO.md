# Drawing Tools + Crosshair Fixes — Implementation TODO

## Step 1: Add `HitPart::Edge` to types.rs + cursor helper ✅
- [ ] Add `HitPart::Edge` variant
- [ ] Add `cursor_for_hit(tool, part, anchor_index, anchors)` helper function

## Step 2: Update rectangle.rs — 4-corner anchors + edge detection
- [ ] Expand to 4 corner anchors (TL, TR, BR, BL)
- [ ] Hit-test: corners → `Anchor(i)`, edges → `Edge`, interior → `Body`
- [ ] `cursor_for_anchor()` returns nwse/nesw resize cursors based on corner position

## Step 3: Update interaction.rs — drawing cursor + drag state
- [ ] Add `drawing_cursor: Option<&'static str>` field
- [ ] Add `drawing_drag_active: bool` field
- [ ] Update `cursor_hint()` to check drawing cursor first
- [ ] Suppress crosshair in `pane_pointer_move()` when `drawing_drag_active`

## Step 4: Update drawings/mod.rs — hover cursor + body-drag passthrough
- [ ] Add `hover_cursor()` method that does hit-test and returns cursor string
- [ ] Rectangle body drag: return flag to indicate "pass through to pan"

## Step 5: Update wasm/src/lib.rs — wire up cursor + drag fixes
- [ ] `on_pane_pointer_move()`: hover hit-test → set cursor override
- [ ] `on_pointer_down()`: Rectangle body hit → don't start drag, fall through to pan
- [ ] Drawing drag start: hide crosshair, set drawing_drag_active
- [ ] Drawing drag end: restore crosshair, clear drawing_drag_active

## Step 6: Verify + Build
- [ ] `cargo check --target wasm32-unknown-unknown -p raycore-wasm`
- [ ] `wasm-pack build wasm/ --target web --out-dir ../pkg`
- [ ] Verify demo serves latest
