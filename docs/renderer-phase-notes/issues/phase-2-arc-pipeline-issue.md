# Issue: Phase 2 - Arc Pipeline

Status: done

## Goal

Implement arc GPU pipeline foundations for schematic symbol arcs and curved graphics.

## Scope

- Add arc shader source and validation path.
- Add arc pipeline upload/draw implementation.
- Integrate arc emitter flow from schematic snapshot to scene.
- Add edge-case validation for arc sweep and small radii.

## Checklist

- [x] Implement arc shader and arc pipeline upload/draw path.
- [x] Integrate static arc emitter into schematic scene path.
- [x] Add arc edge-case checks for sweep wrapping and tiny radius.
- [x] Add smoke render for arc path.

## Acceptance criteria

- [x] Arc primitives render correctly at low and high zoom.
- [x] Sweep normalization behaves correctly across wrap boundaries.
- [x] Arc path passes compile and smoke verification.
- [x] Relevant fixture or snapshot checks pass.

## Required evidence notes

Suggested filenames:

- logs/phase-2-task-01-arc-shader-pipeline.md
- logs/phase-2-task-02-arc-emitter-integration.md
- logs/phase-2-task-03-arc-edge-cases.md
- logs/phase-2-task-04-arc-smoke-render.md

## Non-goals

- No PCB 2D runtime implementation.
- No PCB 3D runtime implementation.
- No foreign parser or external GPL-derived code.
