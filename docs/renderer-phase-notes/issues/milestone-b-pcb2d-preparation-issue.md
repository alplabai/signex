# Issue: Milestone B - PCB 2D Renderer Preparation (Sprint B)

Status: done

## Goal

Prepare Milestone B execution boundaries, technical design package, and entry gates before any PCB 2D runtime implementation starts.

## Scope

- Freeze Milestone B scope and non-goals from the renderer plan.
- Define PCB primitive mapping and pass order model.
- Define PCB-specific dirty flags and trigger matrix.
- Define and document the Section 9.3 memory gate architecture and acceptance checks.
- Produce a Sprint C handoff package with Definition of Ready and Definition of Done.

## Checklist

- [x] Task 01: Scope freeze and dependency boundaries are documented.
- [x] Task 02: PCB primitive mapping table is documented.
- [x] Task 03: Pass order and layer ordering rules are documented.
- [x] Task 04: PCB dirty flags and trigger matrix are documented.
- [x] Task 05: Memory gate architecture (tile/LOD/streaming/budget) is documented.
- [x] Task 06: Memory gate validation plan and measurable success criteria are documented.
- [x] Task 07: Sprint C handoff package is documented.

## Acceptance criteria

- [x] Milestone B scope, assumptions, and non-goals are explicit and testable.
- [x] Primitive mapping and pass order can be implemented without specification gaps.
- [x] Dirty-flag model supports incremental uploads for PCB entities.
- [x] Memory gate design satisfies all minimum criteria from Section 9.3.
- [x] Validation plan includes fixture classes, telemetry fields, and pass/fail thresholds.
- [x] Sprint C starts with a clear vertical slice and readiness checklist.

## Required evidence notes

Suggested filenames:

- logs/milestone-b-task-01-scope-freeze.md
- logs/milestone-b-task-02-primitive-mapping.md
- logs/milestone-b-task-03-pass-order-layering.md
- logs/milestone-b-task-04-dirty-flags-trigger-matrix.md
- logs/milestone-b-task-05-memory-gate-architecture.md
- logs/milestone-b-task-06-gate-validation-plan.md
- logs/milestone-b-task-07-sprint-c-handoff.md

## Non-goals

- No PCB 2D runtime shader or pipeline implementation in this sprint.
- No PCB 3D import/runtime execution (Milestone C remains deferred).
- No legacy crate removal or parser migration work.
