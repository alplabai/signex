# Task 03 Log: Projection Texture Pass Integration

Date: 2026-05-05
Branch: feature/v0.12-cleanroom-rewrite

## Scope

Implement the projection texture pass integration and alignment checks for the
PCB 3D runtime (`signex-renderer/src/pcb3d.rs`), establishing the ordering
boundary between the opaque pass (`scene.polygons`) and the projection overlay
pass (`scene.overlay_polygons`).

## Added types

- `ProjectionBounds` — rectangular bounds used for both footprint-space extents
  (mm) and normalized UV coverage ([0.0, 1.0]).
- `ProjectionPassConfig` — configuration: `footprint_bounds`, `uv_bounds`,
  `tile_columns`, `fill_alpha`, `stroke_width_mm`.
- `ProjectionAlignmentError` — three variants: `ZeroAreaFootprint`,
  `UvBoundsOutOfRange`, `UvBoundsInverted`.

## Added functions

- `check_projection_alignment(model_id, config)` — validates footprint area > 0,
  UV values ∈ [0.0, 1.0], UV min < max on both axes.
- `emit_projection_pass(model, theme, scene, config)` — calls alignment check
  first; on success emits one `GpuPolygon` per staged opaque primitive into
  `scene.overlay_polygons`. Does not touch `scene.polygons`.

## Ordering boundary

| Pass           | Target field           |
|----------------|------------------------|
| Opaque pass    | `scene.polygons`       |
| Projection pass| `scene.overlay_polygons` |

This separation is enforced at the API level; callers invoke
`emit_opaque_pass_preview` before `emit_projection_pass`.

## Tests added (7 new)

| Test | Outcome |
|------|---------|
| `pcb3d_projection_alignment_rejects_zero_area_footprint` | ok |
| `pcb3d_projection_alignment_rejects_uv_out_of_range` | ok |
| `pcb3d_projection_alignment_rejects_inverted_uv_bounds` | ok |
| `pcb3d_projection_alignment_accepts_valid_config` | ok |
| `pcb3d_projection_pass_emits_to_overlay_polygons_not_base_polygons` | ok |
| `pcb3d_projection_pass_emits_one_overlay_per_staged_primitive` | ok |
| `pcb3d_projection_pass_returns_error_on_misaligned_config` | ok |

## Full suite result

`cargo test -p signex-renderer`: 41 tests, 0 failed.

## Checklist / issue updates

- Checklist: "Projection texture pass integration completed" → checked.
- Checklist: "Overlay ordering checks for 3D runtime completed" → checked.
- Issue Task 03 → [x].
- Issue acceptance criterion "Projection pass ordering and ownership boundaries
  are implementation-backed" → [x].
