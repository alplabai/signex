# Phase Note

## Metadata

- Phase: Milestone B (Deferred)
- Task ID: 06
- Task name: memory gate validation and benchmark plan
- Owner: renderer-team
- Date: 2026-05-05
- Status: done

## Scope

Define measurable validation for memory gate readiness before PCB runtime implementation.

## Implementation notes

- Defined fixture classes:
  - Small: simple 2-layer board.
  - Medium: mixed-density board with zones and moderate text.
  - Large: high-density board with heavy copper and overlays.
- Defined minimum pass criteria:
  - No out-of-memory failure in large fixture stress run.
  - Budget telemetry reports peak and sustained usage.
  - Fallback behavior is triggered and logged when pressure thresholds are crossed.
- Defined baseline metrics:
  - Peak GPU texture memory.
  - Tile miss ratio.
  - Eviction rate per second.
  - Frame time percentiles (p50, p95, p99).
- Defined planned command set placeholder for execution sprint:
  - cargo test -p signex-renderer pcb_memory_gate -- --nocapture
  - cargo test -p signex-renderer pcb_large_board_stress -- --nocapture
  - cargo bench -p signex-renderer pcb_streaming

## Clean-room evidence

- Source: renderer plan Section 9.3 minimum gate criteria.
- Derivation: criteria expanded into fixture, metric, and threshold definitions.
- Rationale: convert high-level gate language into executable verification work.
- Clean-room check: No GPL-licensed source consulted
- Verification: all Section 9.3 minimum gate criteria are mapped to explicit checks in this note.

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
