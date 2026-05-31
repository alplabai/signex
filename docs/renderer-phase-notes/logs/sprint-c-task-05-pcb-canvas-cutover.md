# Phase Note

## Metadata

- Phase: Sprint C
- Task ID: 05
- Task name: PCB canvas runtime cutover
- Owner: renderer-team
- Date: 2026-05-05
- Status: done

## Scope

Switch `signex-app` PCB canvas draw flow to use `signex-renderer` scene emission path while preserving runtime stability.

## Implementation notes

- Added renderer snapshot state to PCB canvas (`PcbSnapshot`) alongside the legacy snapshot.
- Reworked PCB canvas draw path:
  - Build scene with `PcbRenderer::build_scene`.
  - Emit lines/circles/polygons/overlays through iced canvas primitives.
  - Keep legacy `signex_render::pcb::render_pcb` as fallback path for this slice.
- Updated load gateway PCB sync path to hydrate both legacy and renderer snapshots.
- Added `signex-gfx` dependency in `signex-app` for scene primitive types.

## Clean-room evidence

- Source: Sprint C cutover plan, existing clean-room renderer modules.
- Derivation: app integration uses exported `signex-renderer` + `signex-gfx` APIs only.
- Rationale: enable progressive cutover without breaking PCB runtime interactions.
- Clean-room check: No GPL-licensed source consulted
- Verification: targeted app tests and full `signex-renderer` tests passed.

## Artifacts

- PR/commit: pending
- Test output:
  - `cargo test -p signex-app pcb_dirty_adapter --lib -- --nocapture`
  - `cargo test -p signex-renderer -- --nocapture`
- Screenshot/benchmark: n/a

## Exit checklist

- [x] Implementation completed
- [x] Source and derivation documented
- [x] Clean-room check confirmed
- [x] Verification artifact added
- [x] Linked from issue/checklist
