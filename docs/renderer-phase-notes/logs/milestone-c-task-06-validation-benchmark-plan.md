# Phase Note

## Metadata

- Phase: Milestone C (Preparation)
- Task ID: 06
- Task name: validation and benchmark plan
- Owner: renderer-team
- Date: 2026-05-05
- Status: done

## Scope

Define measurable validation and benchmark plan for Milestone C implementation sprint.

## Implementation notes

- Fixture classes defined:
  - Small: low-component board with 1-2 mechanical models.
  - Medium: mixed board with connector-heavy model set and varied transforms.
  - Large: dense board with high model count and layered projected textures.
- Regression command set documented (planned for execution sprint):
  - `cargo test -p signex-renderer pcb3d_import_contract -- --nocapture`
  - `cargo test -p signex-renderer pcb3d_runtime_glb_ingest -- --nocapture`
  - `cargo test -p signex-renderer pcb3d_projection_parity -- --nocapture`
  - `cargo bench -p signex-renderer pcb3d_camera_orbit`
  - `cargo bench -p signex-renderer pcb3d_large_board_streaming`
- Pass/fail thresholds documented:
  - No runtime source-format parse attempts in GLB ingest tests.
  - Cache hit ratio remains stable for unchanged key tuples.
  - Large fixture run has no OOM failure under configured budget class.
  - Frame-time target tracks p50/p95/p99 and remains within milestone budget envelope.
- Parity checks documented:
  - Identical GLB input yields stable render output across repeated loads.
  - Projection layer alignment remains stable under camera orbit and zoom sweeps.

## Clean-room evidence

- Source: Renderer plan Section 9.2 and risk gates in Section 10.
- Derivation: validation criteria mapped to deferred Milestone C contracts.
- Rationale: ensure runtime/import boundaries are verifiable before implementation.
- Clean-room check: No GPL-licensed source consulted
- Verification: milestone C issue/checklist updated with matching validation plan items.

## Artifacts

- PR/commit: pending
- Test output: documentation-only task
- Screenshot/benchmark: n/a

## Exit checklist

- [x] Implementation completed
- [x] Source and derivation documented
- [x] Clean-room check confirmed
- [x] Verification artifact added
- [x] Linked from issue/checklist
