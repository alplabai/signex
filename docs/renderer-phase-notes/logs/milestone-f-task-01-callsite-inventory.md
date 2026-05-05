# Phase Note

## Metadata

- Phase: Milestone F (Execution)
- Task ID: 01
- Task name: schematic runtime callsite inventory and cutover contract freeze
- Owner: renderer-team
- Date: 2026-05-05
- Status: done

## Scope

Inventory all direct `signex_render::schematic` runtime callsites in `signex-app`
and freeze the first migration boundary for incremental cutover.

## Inventory baseline

Command used:

```text
rg -n "signex_render::schematic::" crates/signex-app/src
```

Result summary:

- Direct runtime callsites: 75
- Source files touched by the callsite map: 13
- Highest density file: `crates/signex-app/src/canvas/mod.rs`

Mapped callsite families:

1. Snapshot/cache types
   - `SchematicRenderCache`, `SchematicRenderSnapshot`, `SchematicSheetExt`
2. Runtime render path
   - `render_schematic(...)`
3. Interaction path
   - `hit_test`, `hit_test_rect_mode`, `hit_test_polygon`, `SelectionMode`
4. Overlay helpers
   - `selection::draw_selection_overlay`, `draw_power_port_preview`,
     `text::draw_text_note_preview`, `label::draw_label_preview`
5. Text helpers and invalidation
   - `text::escape_for_standard`, `text::expand_char_escapes`,
     `RenderInvalidation`

## Cutover contract freeze (Task 01 output)

- Migration starts with path centralization, not immediate API rewrite.
- `signex-app` modules stop referencing `signex_render::schematic` directly.
- A single bridge module in app owns legacy runtime imports during transition.
- Behavior parity checks for canvas + selection remain mandatory before Task 03.

## Clean-room evidence

- Source: Milestone F issue scope and Task 01 definition.
- Derivation: callsite graph derived by repository grep over `signex-app/src`.
- Rationale: centralizing callsites first reduces risk and enables staged swap to
  `signex-renderer` without broad multi-file breakage.
- Clean-room check: No GPL-licensed source consulted.
- Verification: callsite count and file map captured in this note.

## Artifacts

- PR/commit: pending
- Test output: n/a (inventory task)
- Screenshot/benchmark: n/a

## Exit checklist

- [x] Implementation completed
- [x] Source and derivation documented
- [x] Clean-room check confirmed
- [x] Verification artifact added
- [x] Linked from issue/checklist
