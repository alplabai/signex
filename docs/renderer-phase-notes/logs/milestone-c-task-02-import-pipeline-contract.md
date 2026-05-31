# Phase Note

## Metadata

- Phase: Milestone C (Preparation)
- Task ID: 02
- Task name: import pipeline crate contract
- Owner: renderer-team
- Date: 2026-05-05
- Status: done

## Scope

Define crate-level ownership boundaries for `signex-model-import` and its integration contract with runtime 3D renderer.

## Implementation notes

- Defined `signex-model-import` as the only crate responsible for non-GLB source ingestion (`.step`, `.stp`, `.wrl`, `.gltf`, `.glb`).
- Defined runtime handoff artifact as GLB-only payload plus metadata envelope.
- Defined crate boundary split:
  - Import crate owns source parsing, normalization, and conversion pipeline.
  - Runtime 3D renderer owns GLB consumption and draw-pass integration only.
- Defined error boundary:
  - Import-time format/conversion failures surface in import diagnostics.
  - Runtime load failures surface as GLB ingest errors, never as STEP/VRML parse paths.

## Contract sketch

- Input to import crate: source file path and project context metadata.
- Output from import crate: cached GLB path, cache metadata (source path, mtime, converter version).
- Input to runtime renderer: GLB path or in-memory GLB bytes only.

## Clean-room evidence

- Source: Renderer plan Section 9.2 (Milestone C deferred scope).
- Derivation: crate responsibility split directly follows GLB-only runtime rule.
- Rationale: keep runtime deterministic and format-agnostic while isolating converter complexity.
- Clean-room check: No GPL-licensed source consulted
- Verification: milestone C issue/checklist updated with matching contract boundaries.

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
