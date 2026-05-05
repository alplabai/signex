# Issue: Milestone C - PCB 3D and Model Import Preparation

Status: in_progress

## Goal

Prepare Milestone C execution boundaries, architecture contracts, and validation gates for PCB 3D and model-import runtime.

## Scope

- Freeze Milestone C scope and non-goals from the renderer plan.
- Define import pipeline contract for `signex-model-import`.
- Define format conversion and cache policy (`source path + mtime`).
- Define runtime GLB-only ingestion contract for 3D renderer.
- Define hybrid 3D rendering approach for solids and projected board-layer visuals.
- Produce validation and benchmark command plan for Milestone C implementation sprint.

## Checklist

 - [x] Task 01: Scope freeze and dependency boundaries are documented.
 - [x] Task 02: Import pipeline crate contract is documented.
- [x] Task 03: Format conversion and cache-key policy are documented.
- [x] Task 04: Runtime GLB ingestion interface and constraints are documented.
- [x] Task 05: Hybrid 3D rendering pass model is documented.
- [x] Task 06: Milestone C validation and benchmark plan is documented.
- [ ] Task 07: Milestone C implementation handoff package is documented.

## Acceptance criteria

- [ ] Milestone C scope, assumptions, and non-goals are explicit and testable.
- [x] Runtime path clearly accepts GLB only, with no direct STEP/VRML parse in runtime.
- [x] Import conversion and cache policy are deterministic and implementation-ready.
- [x] Hybrid rendering model defines clear pass ordering and data ownership boundaries.
- [x] Validation plan includes fixture classes, parity checks, and measurable thresholds.
- [ ] Milestone C starts with a clear vertical slice and readiness checklist.

## Required evidence notes

Suggested filenames:

- logs/milestone-c-task-01-scope-freeze.md
- logs/milestone-c-task-02-import-pipeline-contract.md
- logs/milestone-c-task-03-conversion-cache-policy.md
- logs/milestone-c-task-04-runtime-glb-ingestion.md
- logs/milestone-c-task-05-hybrid-3d-pass-model.md
- logs/milestone-c-task-06-validation-benchmark-plan.md
- logs/milestone-c-task-07-implementation-handoff.md

## Non-goals

- No direct STEP/VRML parsing in 3D runtime renderer.
- No full schematic renderer migration in this milestone.
- No implicit removal of legacy crates before accepted migration gates are met.
