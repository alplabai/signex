# Phase Note

## Metadata

- Phase: 6
- Task ID: 02
- Task name: viewport culling integration with rstar
- Owner: renderer-team
- Date: 2026-05-05
- Status: done

## Scope

Integrate viewport culling for schematic primitives to stabilize large-sheet frame times.

## Implementation notes

- Added `UploadCulling` and `ViewportAabbMm` models in `scene/upload.rs`.
- Added `apply_dirty_uploads_with_culling` path that integrates viewport filtering into dirty-gated uploads.
- Added rstar-backed envelope indexing and intersection queries for lines, circles, arcs, polygons, text, overlay, and ERC marker batches.
- Added focused tests that verify core primitive filtering, overlay/ERC filtering, and disabled-culling full-batch behavior.

## Clean-room evidence

- Source: rstar public documentation and renderer culling plan.
- Derivation: viewport query mapping to visible primitive subsets.
- Rationale: reduce draw work in dense or zoomed-in scenes.
- Clean-room check: No GPL-licensed source consulted
- Verification: `cargo test -p signex-gfx scene::upload -- --nocapture`, `cargo test -p signex-gfx -- --nocapture`, `cargo test -p signex-renderer -- --nocapture`, `cargo check -p signex-gfx -p signex-renderer`, and `cargo build -p signex-gfx -p signex-renderer` passed.

## Artifacts

- PR/commit: pending
- Test output: signex-gfx tests passed (39 passed, 0 failed), signex-renderer tests passed (9 passed, 0 failed)
- Screenshot/benchmark: pending

## Exit checklist

- [x] Implementation completed
- [x] Source and derivation documented
- [x] Clean-room check confirmed
- [x] Verification artifact added
- [x] Linked from issue/checklist
