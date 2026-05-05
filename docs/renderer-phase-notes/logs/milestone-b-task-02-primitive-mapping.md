# Phase Note

## Metadata

- Phase: Milestone B (Deferred)
- Task ID: 02
- Task name: PCB primitive mapping specification
- Owner: renderer-team
- Date: 2026-05-05
- Status: done

## Scope

Define deterministic PCB entity to GPU primitive mapping for Milestone B.

## Implementation notes

- Defined baseline mapping table:
  - Trace segment: line-strip or segmented line primitive.
  - Via: annulus ring + drill hole circle.
  - Through-hole pad: pad body shape + drill circle.
  - SMD pad: rectangle/rounded-rect/custom polygon.
  - Zone: polygon fill with hole-aware path.
  - Ratsnest: dashed or low-alpha line primitive.
  - DRC marker: token-styled marker primitives + optional text.
  - Rule area: translucent polygon fill + border stroke.
- Marked all severity and status visuals as token-driven style slots.
- Added mapping constraints for incremental updates: each entity family maps to a stable batch group.

## Clean-room evidence

- Source: renderer plan Milestone B required contents.
- Derivation: direct mapping from domain entities to renderer primitive families.
- Rationale: isolate batch families early to simplify dirty upload logic in implementation.
- Clean-room check: No GPL-licensed source consulted
- Verification: mapping definitions are captured in this note and referenced by Task 03 and Task 04.

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
