# Phase Note

## Metadata

- Phase: Milestone C (Preparation)
- Task ID: 01
- Task name: scope freeze and dependency boundaries
- Owner: renderer-team
- Date: 2026-05-05
- Status: done

## Scope

Freeze the exact Milestone C preparation scope so PCB 3D/model-import implementation starts without ambiguity.

## Implementation notes

- Locked in Milestone C scope to PCB 3D + model import pipeline preparation.
- Locked runtime format boundary: runtime renderer accepts GLB only.
- Locked import boundary: STEP/VRML parsing is isolated to import pipeline conversion stage.
- Locked cache contract baseline: cache key uses source path + mtime.
- Confirmed non-goals: no schematic-wide migration and no implicit legacy crate removal in this milestone.

## Clean-room evidence

- Source: Renderer plan Section 9.2 and Section 12.
- Derivation: direct scope extraction from deferred milestone definitions.
- Rationale: prevent runtime/import boundary drift before implementation.
- Clean-room check: No GPL-licensed source consulted
- Verification: milestone C issue/checklist updated with matching scope and non-goals.

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
