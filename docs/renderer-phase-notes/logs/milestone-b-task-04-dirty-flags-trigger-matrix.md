# Phase Note

## Metadata

- Phase: Milestone B (Deferred)
- Task ID: 04
- Task name: PCB dirty flags and trigger matrix
- Owner: renderer-team
- Date: 2026-05-05
- Status: done

## Scope

Define PCB-specific dirty categories and trigger behavior for incremental scene updates.

## Implementation notes

- Proposed dirty categories:
  - TRACES
  - VIAS
  - PADS
  - ZONES
  - RATSNEST
  - DRC
  - RULE_AREAS
  - TEXT
  - THEME
  - CAMERA
- Trigger matrix baseline:
  - Net reroute: TRACES + RATSNEST
  - Via move/add/remove: VIAS + TRACES
  - Footprint move: PADS + TRACES + TEXT
  - Zone refill: ZONES
  - Rule change: RULE_AREAS + DRC
  - Theme swap: THEME only (no geometry rebuild)
  - Pan/zoom: CAMERA only
- Added upload gating rule: only batches touched by corresponding dirty bits may upload.

## Clean-room evidence

- Source: renderer plan Milestone B scope and prior dirty-flag model from schematic milestone.
- Derivation: event-to-batch mapping for minimal upload policy.
- Rationale: protect frame time and upload budget on large boards.
- Clean-room check: No GPL-licensed source consulted
- Verification: trigger matrix is fully covered by planned checks in Task 06.

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
