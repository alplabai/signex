# Phase Note

## Metadata

- Phase: 2
- Task ID: 01
- Task name: arc shader and pipeline foundation
- Owner: renderer-team
- Date: 2026-05-04
- Status: done

## Scope

Start arc pipeline implementation with shader source and upload/draw path.

## Implementation notes

- Added arc WGSL shader source and exported via shader module.
- Implemented ArcPipeline with real GPU upload and draw path.
- Added arc offscreen smoke pass and passing test.

## Clean-room evidence

- Source: IEEE 315 and WGSL public specification.
- Derivation: arc SDF fragment logic with explicit sweep normalization.
- Rationale: support curved symbol geometry as the next primitive class.
- Clean-room check: No GPL-licensed source consulted
- Verification: `cargo test -p signex-gfx arc_smoke_pass_runs -- --nocapture` passed and `cargo check -p signex-gfx -p signex-renderer` passed.

## Artifacts

- PR/commit: local workspace changes, commit pending
- Test output: arc smoke test passed
- Screenshot/benchmark: pending

## Exit checklist

- [x] Implementation completed
- [x] Source and derivation documented
- [x] Clean-room check confirmed
- [x] Verification artifact added
- [x] Linked from issue/checklist
