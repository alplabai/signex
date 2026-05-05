# Issue: Milestone D - `signex-model-import` Pipeline Preparation

Status: done

## Goal

Prepare Milestone D execution boundaries, architecture contracts, and validation
gates for the `signex-model-import` crate — the import pipeline responsible for
converting STEP, VRML, and GLTF source models into cached GLB artifacts consumed
by the PCB 3D runtime.

## Background

Milestone C locked the runtime GLB-only contract and documented the import
pipeline crate boundary (Milestone C prep Task 02 and Task 03). Milestone D
translates those contracts into implementation-ready specifications for
`signex-model-import` as a standalone crate in the workspace.

## Scope

- Freeze Milestone D scope and non-goals.
- Produce a clean-room STEP/P21 format analysis and parser contract.
- Produce a clean-room VRML/WRL format analysis and parser contract.
- Define the GLTF (JSON) → GLB container wrapping contract.
- Define GLB output normalization: coordinate system, units, mesh deduplication.
- Design crate scaffold, error model, and test harness plan.
- Produce Milestone D execution handoff package.

## Task breakdown (ordered)

- [x] Task 01: Scope freeze and dependency boundaries.
- [x] Task 02: STEP/STP source format analysis and parser contract.
- [x] Task 03: VRML/WRL source format analysis and parser contract.
- [x] Task 04: GLTF → GLB container wrapping contract.
- [x] Task 05: GLB output normalization and coordinate system contract.
- [x] Task 06: Crate scaffold, error model, and test harness design.
- [x] Task 07: Milestone D execution handoff package.

## Acceptance criteria

- [x] Milestone D scope, assumptions, and non-goals are explicit and testable.
- [x] Each supported source format has a documented parser contract with clean-room sources.
- [x] GLB output normalization rules are deterministic and implementation-ready.
- [x] Crate boundary between `signex-model-import` and `signex-renderer` remains GLB-only at runtime.
- [x] Error model covers all failure modes across all supported source formats.
- [x] Execution handoff includes first vertical slice, Definition of Ready, and Definition of Done.

## Required evidence notes

Suggested filenames:

- logs/milestone-d-task-01-scope-freeze.md
- logs/milestone-d-task-02-step-parser-contract.md
- logs/milestone-d-task-03-vrml-parser-contract.md
- logs/milestone-d-task-04-gltf-glb-wrapping-contract.md
- logs/milestone-d-task-05-glb-normalization-contract.md
- logs/milestone-d-task-06-crate-scaffold-design.md
- logs/milestone-d-task-07-execution-handoff.md

## Non-goals

- No STEP/VRML parsing inside the runtime renderer (`signex-renderer`).
- No GUI file-picker or import wizard in this milestone (import is triggered programmatically).
- No online or cloud-based conversion pipeline.
- No dependency on OCCT or any GPL-licensed geometry kernel in the crate.
- No mesh simplification or LOD generation in this milestone (deferred to a later Milestone E).
