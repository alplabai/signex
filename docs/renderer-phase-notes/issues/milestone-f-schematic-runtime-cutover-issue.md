# Issue: Milestone F - Schematic Runtime Cutover

Status: in_progress

## Goal

Cut over schematic runtime rendering in `signex-app` from `signex-render` to
`signex-renderer` with behavior parity for drawing, selection overlays,
hit-testing, and invalidation-driven updates.

## Scope

- Freeze cutover boundary and migration order for schematic runtime paths.
- Build a compatibility bridge for shared app-side contracts (snapshot,
  invalidation, style/config hooks, hit-test callsites).
- Replace canvas runtime draw path with `signex-renderer` scene build flow.
- Migrate selection and overlay interaction paths that currently call
  `signex_render::schematic::hit_test` and related helpers.
- Remove direct `signex_render::schematic` runtime usage from `signex-app`.
- Validate parity with regression tests and golden/smoke checks.

## Task breakdown (ordered)

- [x] Task 01: Schematic runtime callsite inventory and cutover contract freeze.
- [x] Task 02: App compatibility bridge for snapshot/invalidation/style contracts.
- [ ] Task 03: Canvas render path cutover to `signex-renderer` scene pipeline.
- [ ] Task 04: Hit-test and selection workflow migration.
- [ ] Task 05: Overlay/preview/text helper migration (`escape`, expansion, ghost paths).
- [x] Task 06: Remove remaining direct legacy runtime imports and remove old source crate.
- [ ] Task 07: Regression parity validation and benchmark smoke gates.

## Acceptance criteria

- [x] Main schematic canvas no longer calls legacy schematic runtime API paths.
- [ ] Selection, lasso, and polygon hit-tests match prior behavior on baseline fixtures.
- [ ] Overlay families (preview/ghost/lasso/snap/ERC markers) are emitted with parity in expected layers.
- [ ] App dispatch and invalidation flow maps correctly to renderer dirty-family updates.
- [x] `signex-app` has no direct legacy schematic runtime dependency at cutover exit gate.
- [ ] Cutover regression command set passes locally (`signex-app` + `signex-renderer` test suites).

## Required evidence notes

Suggested filenames:

- logs/milestone-f-task-01-callsite-inventory.md
- logs/milestone-f-task-02-compat-bridge.md
- logs/milestone-f-task-03-canvas-cutover.md
- logs/milestone-f-task-04-hittest-selection-migration.md
- logs/milestone-f-task-05-overlay-helper-migration.md
- logs/milestone-f-task-06-legacy-runtime-removal.md
- logs/milestone-f-task-07-regression-smoke.md

Completed in this slice:

- [x] [logs/milestone-f-task-01-callsite-inventory.md](../logs/milestone-f-task-01-callsite-inventory.md)
- [x] [logs/milestone-f-task-02-compat-bridge.md](../logs/milestone-f-task-02-compat-bridge.md)
- [x] [logs/milestone-f-task-06-legacy-runtime-removal.md](../logs/milestone-f-task-06-legacy-runtime-removal.md)

In progress:

- [ ] [logs/milestone-f-task-03-canvas-cutover.md](../logs/milestone-f-task-03-canvas-cutover.md)

## Non-goals

- No PCB 2D runtime migration in this milestone.
- No PCB 3D or model-import runtime changes in this milestone.
- No redesign of UI workflows unrelated to renderer runtime ownership.
- No PCB/runtime scope expansion beyond schematic migration boundaries.
