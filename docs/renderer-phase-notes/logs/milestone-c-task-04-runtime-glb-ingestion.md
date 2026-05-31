# Phase Note

## Metadata

- Phase: Milestone C (Preparation)
- Task ID: 04
- Task name: runtime GLB ingestion interface and constraints
- Owner: renderer-team
- Date: 2026-05-05
- Status: done

## Scope

Define runtime-side GLB ingestion contract and hard constraints for Milestone C 3D renderer integration.

## Implementation notes

- Runtime input contract locked to GLB-only payloads.
- Runtime interface baseline documented:
  - `model_id`: stable project-scoped identifier.
  - `glb_source`: cached GLB file path or in-memory bytes.
  - `transform`: model-to-board transform payload.
  - `material_policy`: runtime material remap flags.
- Runtime ingestion constraints documented:
  - Runtime rejects `.step`, `.stp`, and `.wrl` payloads by interface contract.
  - Runtime performs GLB validation (asset version, node graph presence, mesh count sanity).
  - Runtime must not invoke source-format converters.
- Error model documented:
  - Invalid GLB => runtime ingest error with model_id and source reference.
  - Missing GLB cache entry => import-cache miss diagnostic, no fallback to source parsing.

## Interface guardrails

- GLB ingestion APIs are pure runtime consumers and must remain converter-agnostic.
- Any source-format conversion responsibility remains in import pipeline crate.
- Runtime telemetry emits ingest success/failure counts and rejected-format attempts.

## Clean-room evidence

- Source: Renderer plan Section 9.2 notes (runtime renderer input: GLB only).
- Derivation: direct interface constraints from GLB-only runtime rule.
- Rationale: keep runtime stable, fast, and decoupled from conversion complexity.
- Clean-room check: No GPL-licensed source consulted
- Verification: milestone C issue/checklist updated with matching runtime ingestion contract.

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
