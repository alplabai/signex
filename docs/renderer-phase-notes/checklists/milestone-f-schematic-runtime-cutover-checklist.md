# Milestone F Schematic Runtime Cutover Checklist

## Preparation

- [ ] Current schematic runtime callsites mapped in `signex-app`.
- [ ] `signex-renderer::schematic` contract and dirty-family behavior reviewed.
- [ ] Cutover guardrail approved: migrate in slices, not one-shot rewrite.

## Implementation slices

- [ ] Task 01: Callsite inventory and cutover contract freeze completed.
- [ ] Task 02: Compatibility bridge module integrated into app runtime.
- [ ] Task 03: Canvas render path switched to `signex-renderer` scene flow.
- [ ] Task 04: Hit-test and selection workflow switched to new runtime bridge.
- [ ] Task 05: Overlay and text helper paths switched.
- [ ] Task 06: Legacy schematic runtime imports removed from app modules.

## Validation

- [ ] Task 07: Regression parity checks pass for schematic interactions.
- [ ] `cargo test -p signex-renderer` passes.
- [ ] Targeted `signex-app` regression tests for canvas/selection/overlay pass.

## Exit gate

- [ ] Task 01-07 complete with evidence logs.
- [ ] Milestone F issue acceptance criteria all checked.
- [ ] Schematic runtime cutover marked done without behavior regressions.
