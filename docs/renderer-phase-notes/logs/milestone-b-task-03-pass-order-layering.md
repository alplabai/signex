# Phase Note

## Metadata

- Phase: Milestone B (Deferred)
- Task ID: 03
- Task name: render pass order and layer ordering
- Owner: renderer-team
- Date: 2026-05-05
- Status: done

## Scope

Define rendering pass order and z-order rules for PCB 2D output determinism.

## Implementation notes

- Proposed pass order:
  1. Board background and paper context.
  2. Static copper geometry (bottom to top by layer order).
  3. Drill and cutout visualization.
  4. Pads and vias emphasis pass.
  5. Zones and keepout visuals.
  6. Ratsnest and interactive guides.
  7. DRC/rule-area overlays.
  8. Text and annotations compositing pass.
- Defined ordering rule: overlays are above static geometry, text remains top-most for readability.
- Added tie-break policy: entity id stable sort inside same layer group to keep deterministic output.

## Clean-room evidence

- Source: renderer plan Milestone B scope and existing schematic overlay ordering policy.
- Derivation: pass decomposition by interaction criticality and readability.
- Rationale: preserve visual hierarchy and deterministic compositing in snapshots and tests.
- Clean-room check: No GPL-licensed source consulted
- Verification: ordering rules align with Task 02 mappings and Task 06 validation expectations.

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
