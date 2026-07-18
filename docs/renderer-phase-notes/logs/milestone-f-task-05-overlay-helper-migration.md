# Phase Note

## Metadata

- Phase: Milestone F (Execution)
- Task ID: 05
- Task name: overlay and text helper migration
- Owner: renderer-team
- Date: 2026-05-06
- Status: done

## Scope

Complete overlay/helper migration in app schematic runtime so transient preview,
selection, and marker visuals flow through the renderer snapshot scene path.

## Implementation summary

- Added shared `draw_renderer_snapshot(...)` helper in
  `crates/signex-app/src/schematic_runtime.rs` and reused it for runtime scene
  emission.
- Migrated overlay families to renderer snapshot overlay buckets:
  - selection overlay (`preview_lines`, `ghost_polygons`, `lasso_lines`,
    `snap_circles`)
  - ERC marker overlay
- Migrated helper preview drawing to renderer scene flow:
  - `draw_power_port_preview(...)`
  - `text::draw_text_note_preview(...)`
  - `label::draw_label_preview(...)`

## Verification

Commands:

```text
cargo check -p signex-app
cargo test -p signex-app schematic_runtime::tests
```

Result:

- `cargo check -p signex-app`: pass (warnings only).
- `cargo test -p signex-app schematic_runtime::tests`: pass
  (`3 passed; 0 failed`).

## Clean-room evidence

- Source: Milestone F overlay parity requirements and app-local runtime APIs.
- Derivation: overlay/text helpers rebuilt on `signex-renderer` snapshot flow.
- Clean-room check: no legacy GPL schematic runtime helper code used.

## Exit checklist

- [x] Overlay helper paths emit through renderer snapshot scene flow
- [x] Text preview helpers emit through renderer snapshot text pipeline
- [x] Build/test validation completed
- [x] Evidence log added
