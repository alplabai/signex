# Issue: Sprint C - PCB 2D Runtime Execution and Cutover

Status: in_progress

## Goal

Execute Sprint C as incremental vertical slices to complete PCB 2D runtime integration and reach a clean removal gate for legacy PCB render usage.

## Scope

- Finalize PCB 2D scene translation path in `signex-renderer` with deterministic outputs.
- Integrate app-side dirty-event routing with renderer slice families.
- Cut over `signex-app` PCB canvas runtime path from legacy `signex-render::pcb` to `signex-renderer` scene path.
- Prepare and validate dependency-removal gate for legacy PCB render APIs.

## Task breakdown (ordered)

- [x] Task 01: Base PCB vertical slice (traces, vias, pads) with dirty flags and fixture/golden tests.
- [x] Task 02: Zones, rule areas, ratsnest, and DRC overlay slices.
- [x] Task 03: Deterministic zone compositing order hardening and benchmark fixture guards.
- [x] Task 04: App dirty-event adapter bridge (`Message`/`CanvasEvent` -> `PcbAppEvent`).
- [x] Task 05: PCB canvas runtime cutover to `signex-renderer` scene build/render flow.
- [x] Task 06: Remove direct `signex_render::pcb` usage from `signex-app` and validate behavior parity.
- [ ] Task 07: Legacy cleanup gate for PCB path (`Cargo.toml` dependency, dead helpers, regression checks).

## Acceptance criteria

- [x] Deterministic fixture + golden tests exist and are green for implemented PCB slices.
- [x] Dirty trigger matrix is mapped and test-verified for PCB event families.
- [x] `signex-app` PCB runtime no longer depends on `signex_render::pcb` symbols.
- [ ] PCB interaction parity (selection/camera/overlay visibility) remains stable after cutover.
- [ ] Regression commands are documented and pass in CI-equivalent local run.

## Required evidence notes

Suggested filenames:

- logs/sprint-c-task-01-base-vertical-slice.md
- logs/sprint-c-task-02-overlays-zones.md
- logs/sprint-c-task-03-zone-compositing-benchmark.md
- logs/sprint-c-task-04-app-dirty-adapter.md
- logs/sprint-c-task-05-pcb-canvas-cutover.md
- logs/sprint-c-task-06-legacy-pcb-api-removal.md
- logs/sprint-c-task-07-cleanup-and-regression.md

## Non-goals

- No PCB 3D import/runtime work in Sprint C.
- No full schematic renderer cutover in this sprint.
- No wholesale deletion of legacy `signex-render` crate before PCB cutover exit gate passes.
