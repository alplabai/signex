# Symbol Editor — Salvage Plan (`feature/v0.13-symbol` → `dev`)

> **Date:** 2026-07-05
> **Context:** `feature/v0.13-symbol` (`36c22ea`) and `dev` diverged at v0.11
> (`#76`) and evolved as two parallel clean-room lines. `dev` is **ahead** on
> the footprint editor (862 KB vs 362 KB) and sketch solver (325 KB vs 179 KB),
> but **behind** on the **symbol editor** (111 KB vs 160 KB). This plan brings
> the symbol-editor improvements forward **without** regressing dev's footprint
> / solver work — so a wholesale merge is the wrong move.

## 1. Method verdict: reimplement, do **not** cherry-pick

The symbol editor diverged too far for `git cherry-pick` to apply cleanly:

| File | `dev` | `v0.13-symbol` | changed lines |
|------|-------|----------------|---------------|
| `symbol/canvas.rs` | 905 | 1980 | **1763** (≈ full rewrite) |
| `symbol/state.rs` | 836 | 1522 | heavily diverged |
| `active_bar_dropdowns.rs` | 394 | 394 | identical |

A cherry-pick of the v0.13 symbol commits would conflict against dev's
clean-room symbol editor on nearly every line. **Use `v0.13-symbol` as a
functional reference and port each capability onto dev's structure.**

## 2. Missing capabilities

dev's symbol message surface (30 variants) is a strict **subset** of
`v0.13-symbol`'s (35). But the message diff is only the *skeleton* — each
capability sits on a body of supporting functions. A function-level diff of
`symbol/{canvas,state}.rs` shows **~40 functions** present in `v0.13-symbol`
and absent in dev, and even that is a floor (behavioural fixes *inside*
shared functions — the 1763-line `canvas.rs` divergence — are not captured
by a name diff). **"Salvage all of it" is not a one-shot guarantee; the
reliable path is capability-by-capability port + test + you running each
slice to confirm.**

| Capability | Missing variant(s) | Supporting fns in `v0.13-symbol` | Method | Effort |
|-----------|--------------------|----------------------------------|--------|--------|
| **Undo / Redo** | `SymbolUndo`, `SymbolRedo` | handler `dispatch/library.rs:4022`; snapshot stack `documents.rs:251` | reimplement (mirror footprint pattern) | **M** |
| **Rotate selection** | `SymbolRotateSelected` | ~10: `rotate_selected*`, `rotate_selected_with_pivot`, `rotate_graphic_point_`, `rotate_pin_orientation_`, `geometry_center_local`, `pose`/`set_pose`, `pin_body_delta` (+ tests) | reimplement | **M** |
| **Box / rubber-band select** | `SymbolDragCommit` | ~7: `select_in_box`, `graphic_fully_inside_box`, `graphic_intersects_box`, `segment_crosses_box`, `segments_intersect`, `point_in_box`, `world_point`; canvas `mouse_interaction`, `is_graphic_selected`, `item_in_selection`, `selection_anchor` | reimplement | **M–L** |
| **Multi-select move / drag whole symbol** | `SymbolMoveAll` | `move_all`, `move_multiple`, `translate_graphic_by`, `snap_axis_value` | reimplement | **M** |
| **Renderer/geometry helpers** | — | `draw_symbol_with_renderer`, `build_symbol_renderer_snapshot`, `stroke_px_at_zoom`, `stroke_world_mm`, `text_size_px_from_mm`, `screen_px_to_world_mm`, `unwrap_angle`, `circle_vertices`, `to_rgba` | port only what the above need | **varies** |

> **Honesty note:** the ~40-function catalog is a better floor than the
> 5-variant one, but a *complete* behavioural parity check requires diffing
> the bodies of the shared `canvas.rs`/`state.rs` functions too. Treat this
> plan as "known-missing capabilities"; run each ported slice against the
> real app to catch what a static diff can't.

## 3. Symbol fix commits to review (bugs dev's rewrite may share)

`v0.13-symbol` carries geometry/render fixes that dev's clean-room symbol
renderer may have **re-introduced** (or fixed independently). Each needs a
one-off check against dev before porting:

- Prevent **arc discontinuity** when sweeping past ±180°
- Negate **arc angles** in scene renderer + preview for screen-space Y-flip
- Correct **pin text rotation** for screen Y-flip
- Rotate **pin tip** around body-end using B-type pivot (`anchor2d`)
- Reverse **name h_align** for flipped pin orientations (Left/Down)
- `LineJoin::Round` for rectangle/polygon corners
- Clicking a graphic selects **only that graphic**, not everything
- Normalize rotation angles; remove default pin from `Symbol::empty`
- Snapped vs unsnapped coords for drag anchor / hit-testing

> Note: two of these depend on `signex-types/src/{anchor2d,rotation2d}.rs`
> (present in `v0.13-symbol`, absent in dev). dev uses its own `pivot`-based
> rotation — port the *fix logic*, not the util, unless a util is cleaner.

## 4. Priority order (roadmap)

1. **Undo / Redo** — highest value; every editor needs it and dev's symbol
   editor has none. Tractable because dev's **footprint** editor already
   proves the pattern (§5).
2. **Rotate selection** — small, high value (`SymbolRotateSelected`).
3. **Box-select + drag** (`SymbolDragCommit` / `SymbolMoveAll`) — UX polish.
4. **Geometry/render fixes** — verify each against dev, port the ones dev
   still has (arc Y-flip and pin rotation are the likeliest to matter).

## 5. Undo/Redo reimplementation sketch (mirror the footprint pattern)

dev's footprint editor already has a working, proven editor-undo mechanism:

```rust
// documents.rs (footprint):
pub history: Vec<FootprintHistorySnapshot>,   // pre-mutation snapshots
// apply.rs (footprint):
if mutates_footprint_state(&msg) { editor.push_history(); }
```

Mirror it for the symbol editor on dev's clean-room base:

1. Add `history` (+ redo lineage) to dev's `SymbolEditorState` (`state.rs`),
   snapshotting `Symbol` (or an inverse patch).
2. In `symbol/apply.rs::apply_symbol_primitive_edit` (already extracted in
   the #98 refactor — a clean home), gate a `push_history()` on a
   `mutates_symbol_state(&msg)` predicate, exactly like the footprint reducer.
3. Add `SymbolUndo` / `SymbolRedo` variants + handler arms that pop/replay.
4. Wire `standalone.rs` `CanvasAction::Undo → SymbolUndo` (as v0.13 does).
5. Tests: undo-then-redo restores the symbol; redo stack cleared on new edit;
   history cap. (dev's `symbol/tests.rs` is the home.)

Because the pattern is copy-shaped from footprint, this is the lowest-risk,
highest-value first slice.

## 6. What NOT to bring over

- Footprint / sketch / PCB / renderer commits from `v0.13-symbol` — dev is
  **ahead** there; porting them would regress dev.
- `renderer_scene_canvas.rs` — dev uses a different (newer) renderer path.
- Wholesale branch merge — clobbers dev's clean-room footprint + solver.

---

_All figures verified against `origin/dev` (`b849f27`) and
`origin/feature/v0.13-symbol` (`36c22ea`) via message-variant diff and
per-file size/diff comparison._
