# Milestone F Schematic Runtime Cutover Checklist

## Preparation

- [x] Current schematic runtime callsites mapped in `signex-app`.
- [x] `signex-renderer::schematic` contract and dirty-family behavior reviewed.
- [x] Cutover guardrail approved: migrate in slices, not one-shot rewrite.

## Implementation slices

- [x] Task 01: Callsite inventory and cutover contract freeze completed. Evidence: [logs/milestone-f-task-01-callsite-inventory.md](../logs/milestone-f-task-01-callsite-inventory.md)
- [x] Task 02: Compatibility bridge module integrated into app runtime. Evidence: [logs/milestone-f-task-02-compat-bridge.md](../logs/milestone-f-task-02-compat-bridge.md)
- [x] Task 03: Canvas render path switched to `signex-renderer` scene flow. Evidence: [logs/milestone-f-task-03-canvas-cutover.md](../logs/milestone-f-task-03-canvas-cutover.md)
- [x] Task 04: Hit-test and selection workflow switched to new runtime bridge. Evidence: [logs/milestone-f-task-04-hittest-selection-migration.md](../logs/milestone-f-task-04-hittest-selection-migration.md)
- [x] Task 05: Overlay and text helper paths switched. Evidence: [logs/milestone-f-task-05-overlay-helper-migration.md](../logs/milestone-f-task-05-overlay-helper-migration.md)
- [x] Task 06: Legacy runtime imports removed and old source crate deleted. Evidence: [logs/milestone-f-task-06-legacy-runtime-removal.md](../logs/milestone-f-task-06-legacy-runtime-removal.md)

## Validation

- [ ] Task 07: Regression parity checks pass for schematic interactions.
- [ ] `cargo test -p signex-renderer` passes.
- [x] Targeted `signex-app` regression test target compiles (`--no-run`).
- [x] Focused runtime parity tests pass (`cargo test -p signex-app schematic_runtime::tests`).

## Exit gate

- [ ] Task 01-07 complete with evidence logs.
- [ ] Milestone F issue acceptance criteria all checked.
- [ ] Schematic runtime cutover marked done without behavior regressions.
