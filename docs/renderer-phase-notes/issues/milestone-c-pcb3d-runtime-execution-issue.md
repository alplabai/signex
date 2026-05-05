# Issue: Milestone C - PCB 3D Runtime Execution

Status: in_progress

## Goal

Execute Milestone C implementation slices for PCB 3D runtime using the GLB-only ingestion boundary finalized in preparation.

## Scope

- Implement runtime GLB ingest adapter and validation hooks.
- Wire mesh staging into opaque rendering pass ownership.
- Implement projected board-layer pass integration and alignment checks.
- Validate baseline runtime behavior and benchmark gates for Milestone C fixtures.

## Task breakdown (ordered)

- [x] Task 01: Runtime GLB ingest adapter and validation hooks.
- [x] Task 02: Mesh staging and opaque pass wiring.
- [x] Task 03: Projection texture pass integration and alignment checks.
- [ ] Task 04: Integration validation and benchmark smoke gates.

## Acceptance criteria

- [x] Runtime rejects non-GLB path sources and reports cache misses deterministically.
- [x] Runtime validates GLB container version, scene graph presence, and mesh-count sanity.
- [x] Runtime mesh staging path feeds opaque pass without source-format parsing.
- [x] Projection pass ordering and ownership boundaries are implementation-backed.
- [ ] Integration and benchmark commands pass for baseline fixture tiers.

## Required evidence notes

Suggested filenames:

- logs/milestone-c-exec-task-01-runtime-glb-ingest-adapter.md
- logs/milestone-c-exec-task-02-mesh-staging-opaque-pass.md
- logs/milestone-c-exec-task-03-projection-pass-integration.md
- logs/milestone-c-exec-task-04-validation-benchmark-smoke.md

## Non-goals

- No direct runtime parsing of source CAD formats.
- No import-pipeline converter ownership inside runtime module.
- No extension of schematic runtime scope in this issue.
