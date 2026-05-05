# Phase Note

## Metadata

- Phase: Sprint C
- Task ID: 06
- Task name: legacy PCB API removal in app
- Owner: renderer-team
- Date: 2026-05-05
- Status: done

## Scope

Remove direct `signex_render::pcb` usage from `signex-app` and keep PCB runtime on the new renderer scene path.

## Implementation notes

- Removed legacy PCB snapshot field and methods from `PcbCanvas`.
- Removed legacy draw fallback (`signex_render::pcb::render_pcb`) from PCB canvas draw flow.
- Implemented renderer-snapshot-based board-fit bounds calculation.
- Updated load gateway PCB accessors and sync paths to use board + `PcbSnapshot` only.
- Kept schematic-side legacy renderer usage intact (non-goal for this task).

## Clean-room evidence

- Source: Sprint C issue scope and Task 05 cutover baseline.
- Derivation: removal driven by direct symbol search and compile-safe refactor.
- Rationale: isolate PCB runtime from legacy API before broader cleanup gate.
- Clean-room check: No GPL-licensed source consulted
- Verification: `rg` confirms zero `signex_render::pcb` usage in app source; app + renderer tests pass.

## Artifacts

- PR/commit: pending
- Test output:
  - `cargo test -p signex-app pcb_dirty_adapter --lib -- --nocapture`
  - `cargo test -p signex-renderer -- --nocapture`
  - `rg -n "signex_render::pcb" crates/signex-app/src | wc -l` -> `0`
- Screenshot/benchmark: n/a

## Exit checklist

- [x] Implementation completed
- [x] Source and derivation documented
- [x] Clean-room check confirmed
- [x] Verification artifact added
- [x] Linked from issue/checklist
