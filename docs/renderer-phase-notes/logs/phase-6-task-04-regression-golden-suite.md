# Phase Note

## Metadata

- Phase: 6
- Task ID: 04
- Task name: regression and golden render suite
- Owner: renderer-team
- Date: 2026-05-05
- Status: done

## Scope

Add deterministic regression and golden-render validation for renderer hardening.

## Implementation notes

- Added a JSON baseline fixture at `crates/signex-gfx/tests/golden/phase6_regression_golden.json`.
- Added integration suite at `crates/signex-gfx/tests/regression_golden.rs` to validate smoke reports and upload gating counters against the fixture.
- Added culling fixture coverage that verifies visible primitive counts for core and overlay/ERC batches under a deterministic viewport.
- Added theme-only regression check that asserts no geometry uploads during `DirtyFlags::THEME` refresh.

## Clean-room evidence

- Source: project QA policy and renderer phase plan.
- Derivation: fixture-driven deterministic output verification.
- Rationale: reduce release risk by catching visual regressions early.
- Clean-room check: No GPL-licensed source consulted
- Verification: `cargo test -p signex-gfx regression_golden -- --nocapture`, `cargo test -p signex-gfx -- --nocapture`, `cargo test -p signex-renderer -- --nocapture`, `cargo check -p signex-gfx -p signex-renderer`, and `cargo build -p signex-gfx -p signex-renderer` passed.

## Artifacts

- PR/commit: pending
- Test output: regression golden suite passed (2 passed, 0 failed), full signex-gfx tests passed, signex-renderer tests passed.
- Screenshot/benchmark: n/a (offscreen deterministic fixture suite)

## Exit checklist

- [x] Implementation completed
- [x] Source and derivation documented
- [x] Clean-room check confirmed
- [x] Verification artifact added
- [x] Linked from issue/checklist
