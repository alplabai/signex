# Milestone C PCB 3D Runtime Execution Checklist

## Preparation

- [x] Milestone C preparation package reviewed.
- [x] First implementation slice selected from handoff package.
- [x] GLB-only runtime contract confirmed.

## Implemented slices

- [x] Runtime GLB ingest adapter module added to `signex-renderer`.
- [x] Runtime GLB ingest error model mapped to contract failures.
- [x] Integration tests cover reject/accept flows for bytes and cached file-path sources.

## Remaining runtime slices

- [ ] Mesh staging and opaque pass wiring completed.
- [ ] Projection texture pass integration completed.
- [ ] Overlay ordering checks for 3D runtime completed.

## Validation

- [x] `cargo test -p signex-renderer pcb3d_runtime_glb_ingest -- --nocapture` passes.
- [ ] Full Milestone C runtime integration and benchmark command set passes.

## Exit gate

- [ ] Task 01-04 completed from Milestone C execution issue.
- [ ] Milestone C runtime execution issue marked done with evidence logs.
