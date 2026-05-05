# Phase Note

## Metadata

- Phase: 6
- Task ID: 01
- Task name: dirty-flag upload gating
- Owner: renderer-team
- Date: 2026-05-05
- Status: done

## Scope

Implement upload gating so only dirty primitive groups are uploaded.

## Implementation notes

- Added `scene/upload.rs` with `SceneUploadTarget` and `apply_dirty_uploads` to centralize dirty-flag upload gating.
- Added `UploadCounters` instrumentation to track updates per primitive category.
- Implemented dirty mapping for `LINES`, `CIRCLES`, `ARCS`, `POLYGONS`, `TEXT`, `GRID`, `OVERLAY`, and `THEME`.
- Added unit tests that validate no-op behavior, selective dirty behavior, overlay expansion behavior, and error propagation in text upload.

## Clean-room evidence

- Source: dirty-flag design and renderer phase plan.
- Derivation: conditional upload paths keyed by dirty masks.
- Rationale: improve incremental performance and reduce GPU traffic.
- Clean-room check: No GPL-licensed source consulted
- Verification: `cargo test -p signex-gfx scene::upload -- --nocapture`, `cargo test -p signex-gfx -- --nocapture`, `cargo test -p signex-renderer -- --nocapture`, `cargo check -p signex-gfx -p signex-renderer`, and `cargo build -p signex-gfx -p signex-renderer` passed.

## Artifacts

- PR/commit: pending
- Test output: signex-gfx tests passed (34 passed, 0 failed), signex-renderer tests passed (9 passed, 0 failed)
- Screenshot/benchmark: pending

## Exit checklist

- [x] Implementation completed
- [x] Source and derivation documented
- [x] Clean-room check confirmed
- [x] Verification artifact added
- [x] Linked from issue/checklist
