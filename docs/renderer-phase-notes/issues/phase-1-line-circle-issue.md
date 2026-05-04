# Issue: Phase 1 - Line and Circle Pipelines

Status: done

## Goal

Implement line and circle GPU pipeline foundations for schematic wires, buses, and junctions.

## Scope

- Implement line shader and line pipeline upload/draw path.
- Implement circle shader and circle pipeline upload/draw path.
- Add wire color resolution chain for overrides and fallback.
- Integrate static emit functions for wires and junctions in schematic scene building.

## Checklist

- [x] Implement line shader and line pipeline for SDF anti-aliased segments.
- [x] Implement circle shader and circle pipeline for filled/stroked circles.
- [x] Implement wire color resolution fallback order.
- [x] Integrate emit_wires and emit_junctions into schematic scene path.

## Acceptance criteria

- [x] Wires and buses render consistently across zoom levels.
- [x] Junctions render correctly with expected visual sizing.
- [x] Per-wire color override order behaves correctly.
- [x] Relevant fixture or snapshot checks pass.

## Required evidence notes

Create one note per checklist item under logs/ using the phase template.

Suggested filenames:

- logs/phase-1-task-01-line-pipeline.md
- logs/phase-1-task-02-circle-pipeline.md
- logs/phase-1-task-03-wire-color-resolution.md
- logs/phase-1-task-04-static-emitter-integration.md

## Non-goals

- No PCB 2D runtime implementation.
- No PCB 3D runtime implementation.
- No foreign parser or external GPL-derived code.
