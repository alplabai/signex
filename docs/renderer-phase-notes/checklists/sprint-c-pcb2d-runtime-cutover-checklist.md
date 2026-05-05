# Sprint C PCB 2D Runtime Cutover Checklist

## Preparation

- [x] Sprint B handoff package reviewed.
- [x] Sprint C vertical-slice order confirmed.
- [x] Existing PCB fixture/golden baseline verified green.

## Implemented slices (completed)

- [x] Traces/vias/pads scene emission path implemented.
- [x] Zones/rule-areas/ratsnest/DRC overlays implemented.
- [x] Zone compositing order hardened with benchmark fixture guard.
- [x] App dirty hint adapter integrated into dispatch path.

## Cutover tasks

- [ ] Replace `signex_render::pcb::PcbRenderSnapshot` usage in app PCB canvas state.
- [x] Replace legacy PCB draw call path with `signex-renderer` scene build path.
- [x] Route runtime invalidation updates through new PCB dirty family mapping.
- [ ] Remove remaining direct legacy PCB API calls in app PCB modules.

## Validation

- [x] `cargo test -p signex-renderer -- --nocapture` passes.
- [x] Targeted app tests for PCB dirty adapter and PCB canvas interactions pass.
- [ ] Snapshot/fixture parity checks confirm no regression in overlay/zone ordering.

## Exit gate

- [ ] Task 05-07 completed from Sprint C issue.
- [ ] No `signex_render::pcb` symbol usage remains in `signex-app`.
- [ ] Sprint C issue marked done with evidence logs.
