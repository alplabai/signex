# Phase Note

## Metadata

- Phase: Milestone C (Execution)
- Task ID: 01
- Task name: runtime GLB ingest adapter and validation hooks
- Owner: renderer-team
- Date: 2026-05-05
- Status: done

## Scope

Implement first execution vertical slice for Milestone C by adding runtime GLB ingestion APIs with deterministic validation and contract-aligned errors.

## Implementation notes

- Added new module `crates/signex-renderer/src/pcb3d.rs` with runtime ingestion contract types:
  - `RuntimeGlbIngestRequest`
  - `GlbSource` (file path or in-memory bytes)
  - `ModelTransform`
  - `RuntimeMaterialPolicy`
  - `RuntimeGlbModel` and `RuntimeGlbMetadata`
- Added deterministic failure model via `RuntimeGlbIngestError`:
  - Unsupported source format for non-GLB file paths.
  - Missing GLB cache entry for absent `.glb` files.
  - IO read failure diagnostics.
  - Invalid GLB payload diagnostics.
- Implemented GLB validation hooks for runtime boundary checks:
  - Container magic and version validation.
  - Declared-length integrity validation.
  - JSON chunk presence and JSON parse validation.
  - `asset.version` 2.x enforcement.
  - Scene graph/node presence and mesh-count sanity validation.
- Exported runtime module through `signex-renderer` crate root.

## Clean-room evidence

- Source: Milestone C preparation package (Task 04, Task 05, Task 07 handoff) and public glTF 2.0 GLB container specification.
- Derivation: runtime contract encoded directly as typed request, typed error, and deterministic validator pipeline.
- Rationale: enforce GLB-only runtime boundary before mesh/pipeline rendering slices.
- Clean-room check: No GPL-licensed source consulted
- Verification: targeted runtime ingest integration tests pass with acceptance and rejection scenarios.

## Artifacts

- PR/commit: pending
- Test output:
  - `cargo test -p signex-renderer pcb3d_runtime_glb_ingest -- --nocapture`
- Screenshot/benchmark: n/a

## Exit checklist

- [x] Implementation completed
- [x] Source and derivation documented
- [x] Clean-room check confirmed
- [x] Verification artifact added
- [x] Linked from issue/checklist
