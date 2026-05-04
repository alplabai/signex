# Issue: Phase 3 - Polygon and Text Integration

Status: done

## Goal

Implement polygon and text renderer foundations for schematic filled geometry and labels.

## Scope

- Implement polygon pipeline upload/draw path for pre-tessellated polygons.
- Implement text render path foundation for schematic labels.
- Integrate polygon and text emit flow from schematic snapshot to scene.
- Add edge-case and smoke validation for polygon and text paths.

## Checklist

- [x] Implement polygon shader and polygon pipeline upload/draw path.
- [x] Implement text render path and text pipeline bridge.
- [x] Integrate static polygon/text emitters into schematic scene path.
- [x] Add polygon/text edge-case checks and smoke render verification.

## Acceptance criteria

- [x] Polygon primitives render correctly at low and high zoom.
- [x] Text primitives render with stable position, scale, and color.
- [x] DirtyFlags::POLYGONS and DirtyFlags::TEXT selective rebuild works correctly.
- [x] Polygon/text path passes compile and smoke verification.

## Required evidence notes

Suggested filenames:

- logs/phase-3-task-01-polygon-pipeline.md
- logs/phase-3-task-02-text-render-path.md
- logs/phase-3-task-03-polygon-text-emitter-integration.md
- logs/phase-3-task-04-polygon-text-validation.md

## Non-goals

- No PCB 2D runtime implementation.
- No PCB 3D runtime implementation.
- No foreign parser or external GPL-derived code.
