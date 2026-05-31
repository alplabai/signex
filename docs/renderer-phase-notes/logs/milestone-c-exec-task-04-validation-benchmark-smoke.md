# Task 04 Log: Integration Validation and Benchmark Smoke Gates

Date: 2026-05-05
Branch: feature/v0.12-cleanroom-rewrite
Commit: 20acf2ba

## Scope

Provide integration validation and benchmark smoke gates for Milestone C PCB 3D
runtime. Verify that the full pipeline — ingest → mesh staging → opaque pass →
projection pass — produces stable primitive counts across two canonical fixture
tiers, and that pass separation invariants hold at each boundary.

## Fixture tiers

| Tier | Scenes | Nodes | Meshes | Primitives |
|------|--------|-------|--------|------------|
| S (small)  | 1 | 1 | 1 | 3 |
| M (medium) | 2 | 4 | 3 | 7 |

Tier M scene graph: scene 0 → nodes [0, 1] (mesh 0: 2 prim, mesh 1: 3 prim);
scene 1 → nodes [2, 3] (mesh 2: 2 prim, node 3: no mesh). Total staged: 7.

## Test file added

`crates/signex-renderer/tests/pcb3d_benchmark_smoke.rs`

## Tests (8 total)

| Test | Tier | Verified |
|------|------|---------|
| `benchmark_smoke_tier_s_ingest_metadata_counts` | S | scene/node/mesh/primitive counts |
| `benchmark_smoke_tier_s_opaque_pass_emits_expected_polygon_count` | S | `scene.polygons` = 3, overlay untouched |
| `benchmark_smoke_tier_s_projection_pass_emits_expected_overlay_count` | S | `scene.overlay_polygons` = 3, base untouched |
| `benchmark_smoke_tier_s_full_pipeline_pass_separation_holds` | S | both counts stable after both passes |
| `benchmark_smoke_tier_m_ingest_metadata_counts` | M | scene/node/mesh/primitive counts |
| `benchmark_smoke_tier_m_opaque_pass_emits_expected_polygon_count` | M | `scene.polygons` = 7 |
| `benchmark_smoke_tier_m_projection_pass_emits_expected_overlay_count` | M | `scene.overlay_polygons` = 7 |
| `benchmark_smoke_tier_m_full_pipeline_pass_separation_holds` | M | both counts stable after both passes |

## Pass separation invariant

- Opaque pass writes to `scene.polygons` only.
- Projection pass writes to `scene.overlay_polygons` only.
- Each pass must not disturb the other channel; verified by all full-pipeline tests.

## Full suite result

`cargo test -p signex-renderer`: 49 tests, 0 failed.

Test file breakdown:

| File | Tests |
|------|-------|
| unit tests (pcb + schematic + theme) | 19 |
| no_literal_colors lint | 1 |
| pcb3d_benchmark_smoke | 8 |
| pcb3d_runtime_glb_ingest | 16 |
| pcb_dirty_event_integration | 2 |
| pcb_vertical_slice_golden | 3 |

## Milestone C exit gate status

- Issue: Status → done, all tasks and acceptance criteria checked.
- Checklist: exit gate items checked.
- README: C (exec) row → done.
