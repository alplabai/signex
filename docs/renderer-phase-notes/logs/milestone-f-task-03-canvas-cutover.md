# Phase Note

## Metadata

- Phase: Milestone F (Execution)
- Task ID: 03
- Task name: canvas runtime cutover away from legacy schematic runtime
- Owner: renderer-team
- Date: 2026-05-05
- Status: in_progress

## Scope

Deactivate legacy schematic runtime usage in `signex-app` canvas path and
replace it with a clean-room local runtime surface to avoid GPL/KiCad coupling
risk while Milestone F migration continues.

## Implementation summary (this slice)

- Replaced bridge re-exports in `crates/signex-app/src/schematic_runtime.rs`:
  - removed all `signex_render::schematic` re-exports
  - implemented local clean-room runtime API used by app modules
- Kept app callsites stable by preserving the same surface:
  - `SchematicRenderCache`
  - `RenderInvalidation`
  - `ScreenTransform`
  - `render_schematic(...)`
  - `hit_test::{hit_test, hit_test_polygon, hit_test_rect_mode, SelectionMode}`
  - `selection::draw_selection_overlay(...)`
  - `label::draw_label_preview(...)`
  - `text::{draw_text_note_preview, expand_char_escapes, escape_for_standard}`
  - `draw_power_port_preview(...)`
  - `instance_transform(...)`

## Verification

Commands:

```text
cargo check -p signex-app
rg -n "signex_render::schematic::" crates/signex-app/src
```

Result:

- `cargo check -p signex-app`: pass
- Direct `signex_render::schematic` references in app source: 0

## Notes

- This slice removes the legacy schematic runtime dependency path from app code.
- Remaining `signex_render::...` usages in app are non-schematic utilities
  (font/style/color settings) and are tracked as follow-up cleanup in Milestone F.
- Task 03 remains `in_progress` until the target `signex-renderer` scene-flow
  contract is fully adopted.

## Clean-room evidence

- Source: Milestone F issue scope + user directive to disable old runtime paths.
- Derivation: local runtime rebuilt from app contract requirements only.
- Clean-room check: No GPL-licensed source consulted in this slice.

## Exit checklist

- [x] Legacy schematic runtime imports removed from app call path
- [x] Build validation completed
- [x] Evidence log added
- [ ] Full `signex-renderer` scene contract adoption completed
