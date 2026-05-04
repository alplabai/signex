# Phase Note

## Metadata

- Phase: 1
- Task ID: 04
- Task name: static emitter integration for wires and junctions
- Owner: renderer-team
- Date: 2026-05-04
- Status: done

## Scope

Integrate static wire and junction emit paths into schematic scene building.

## Implementation notes

- Introduced a typed schematic snapshot for emitter inputs.
- Added `emit_wires` and `emit_junctions` helpers.
- Wired `DirtyFlags::LINES` and `DirtyFlags::CIRCLES` into scene rebuild logic.

## Clean-room evidence

- Source: phase plan mapping table for static schematic elements.
- Derivation: direct mapping from wire/junction inputs to scene primitives.
- Rationale: make scene path operational for the first two primitive classes in Phase 1.
- Clean-room check: No GPL-licensed source consulted
- Verification: `cargo check -p signex-gfx -p signex-renderer` and `cargo build -p signex-gfx -p signex-renderer` succeeded.

## Artifacts

- PR/commit: local workspace changes, commit pending
- Test output: check/build succeeded
- Screenshot/benchmark: pending

## Exit checklist

- [x] Implementation completed
- [x] Source and derivation documented
- [x] Clean-room check confirmed
- [x] Verification artifact added
- [x] Linked from issue/checklist
