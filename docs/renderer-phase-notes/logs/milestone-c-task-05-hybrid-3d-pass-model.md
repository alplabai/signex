# Phase Note

## Metadata

- Phase: Milestone C (Preparation)
- Task ID: 05
- Task name: hybrid 3D rendering pass model
- Owner: renderer-team
- Date: 2026-05-05
- Status: done

## Scope

Define hybrid 3D rendering model for Milestone C with explicit pass ordering and data ownership boundaries.

## Implementation notes

- Hybrid model baseline documented:
  - Solids path: rasterized meshes from GLB geometry.
  - Board-layer visuals path: projected textures for PCB layer appearance.
- Pass ordering contract documented:
  1. Depth pre-pass for opaque solids.
  2. Opaque solid shading pass.
  3. Projected layer texture pass (board-surface aligned).
  4. Transparent/alpha sorted pass.
  5. Overlay annotation pass (selection/highlight helpers).
- Ownership boundaries documented:
  - Geometry ownership: runtime GLB ingest and mesh staging.
  - Projection ownership: board-layer texture provider and UV/projection binding.
  - Interaction overlay ownership: UI/runtime bridge, not import converter.
- Dirty/update boundaries documented:
  - Model transform/material updates do not force projection texture rebuild.
  - Layer texture updates do not force mesh re-import.

## Boundary assumptions

- Runtime never back-parses source CAD formats.
- Import and runtime remain decoupled through GLB + metadata handoff.
- Projection textures are versioned independently from mesh cache artifacts.

## Clean-room evidence

- Source: Renderer plan Section 9.2 (hybrid 3D approach note).
- Derivation: pass decomposition from solids + projected-layer split.
- Rationale: preserve visual fidelity while isolating complexity by pass.
- Clean-room check: No GPL-licensed source consulted
- Verification: milestone C issue/checklist updated with matching hybrid pass model scope.

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
