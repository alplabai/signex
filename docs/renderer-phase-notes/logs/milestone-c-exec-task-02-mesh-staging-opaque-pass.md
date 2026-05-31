# Phase Note

## Metadata

- Phase: Milestone C (Execution)
- Task ID: 02
- Task name: mesh staging and opaque pass wiring
- Owner: renderer-team
- Date: 2026-05-05
- Status: done

## Scope

Implement mesh staging from validated GLB scene graph data and wire staged opaque primitives into renderer scene polygons for runtime pass ownership.

## Implementation notes

- Extended runtime GLB ingest output in `crates/signex-renderer/src/pcb3d.rs`:
  - `RuntimeMeshStaging` with staged `RuntimeOpaquePrimitive` entries.
  - `RuntimeGlbMetadata` now tracks `mesh_primitive_count` and `opaque_instance_count`.
- Added deterministic scene-graph staging logic:
  - Mesh primitive layouts collected from `meshes[].primitives[]`.
  - Node traversal stages primitives from `nodes[].mesh` references.
  - Child-node traversal preserves deterministic DFS ordering.
  - Out-of-range node/mesh references return explicit ingest errors.
- Added opaque pass wiring helper:
  - `emit_opaque_pass_preview(model, theme, scene, layout)` emits one polygon per staged opaque primitive into `Scene::polygons`.
  - Uses resolved theme slots only; no literal runtime colors introduced.

## Clean-room evidence

- Source: Milestone C execution issue Task 02, Milestone C preparation handoff package, public glTF 2.0 container model.
- Derivation: staged primitive mapping follows `scenes -> nodes -> meshes -> primitives` graph ownership and feeds opaque-pass geometry bucket.
- Rationale: establish typed mesh ownership and deterministic pass input before projection-pass integration.
- Clean-room check: No GPL-licensed source consulted
- Verification:
  - `cargo test -p signex-renderer --test pcb3d_runtime_glb_ingest -- --nocapture`
  - `cargo test -p signex-renderer -- --nocapture`

## Artifacts

- PR/commit: pending
- Screenshot/benchmark: n/a

## Exit checklist

- [x] Implementation completed
- [x] Source and derivation documented
- [x] Clean-room check confirmed
- [x] Verification artifact added
- [x] Linked from issue/checklist
