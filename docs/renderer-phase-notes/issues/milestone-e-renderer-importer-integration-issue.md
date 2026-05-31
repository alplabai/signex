# Issue: Milestone E - Renderer Importer Runtime Integration

Status: not_started

## Goal

Integrate `signex-3d-model-importer` output with `signex-renderer` runtime so
VRML/WRL, GLTF, STEP, and GLB model sources can flow through one deterministic
runtime path while preserving the GLB-only ingest contract inside renderer core.

## Scope

- Define runtime integration contract between importer and renderer boundaries.
- Add conversion-or-pass-through dispatcher before runtime ingest.
- Ensure runtime still ingests GLB only (`ingest_runtime_glb`) after conversion.
- Map importer diagnostics to renderer/app-level user-facing diagnostics.
- Add end-to-end integration tests for source-format to runtime staging flow.
- Add cache-hit validation and benchmark smoke checks for integration path.

## Task breakdown (ordered)

- [ ] Task 01: Integration contract and ownership boundaries (`signex-3d-model-importer` vs `signex-renderer`).
- [ ] Task 02: Source dispatcher (VRML/STEP/GLTF -> importer, GLB -> pass-through).
- [ ] Task 03: Runtime bridge wiring to `ingest_runtime_glb` with cache path handoff.
- [ ] Task 04: Error and warning mapping into renderer diagnostics contract.
- [ ] Task 05: End-to-end integration tests across VRML, GLTF, STEP, GLB paths.
- [ ] Task 06: Cache reuse and integration benchmark smoke gates.

## Acceptance criteria

- [ ] Runtime module keeps GLB-only ingest boundary with no direct STEP/VRML/GLTF parsing.
- [ ] VRML, GLTF, and STEP model sources resolve to cached GLB and ingest successfully.
- [ ] GLB input bypasses conversion and preserves existing runtime behavior.
- [ ] Importer errors and warnings are mapped to stable user-facing diagnostics.
- [ ] Integration test suite covers both conversion path and pass-through path.
- [ ] Cache-hit behavior is verified and benchmark smoke output is recorded.

## Required evidence notes

Suggested filenames:

- logs/milestone-e-task-01-integration-contract.md
- logs/milestone-e-task-02-source-dispatcher.md
- logs/milestone-e-task-03-runtime-bridge.md
- logs/milestone-e-task-04-diagnostics-mapping.md
- logs/milestone-e-task-05-e2e-integration-tests.md
- logs/milestone-e-task-06-cache-benchmark-smoke.md

## Non-goals

- No change to `signex-renderer` GLB container validation rules.
- No new geometry kernels or tessellation algorithms in this issue.
- No UI workflow redesign for model picker/import wizard in this issue.
- No cloud conversion or remote service dependency.
