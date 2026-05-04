# Issue: Phase 0 - Foundation

Status: done

## Goal

Deliver the foundation required to start schematic rendering in a clean-room process.

## Scope

- Build baseline modules for signex-gfx.
- Add signex-renderer iced shader bridge skeleton.
- Establish theme slot infrastructure with no literal color path in renderer core.
- Add clean-room module header template and commit derivation checklist.

## Checklist

- [x] Create signex-gfx foundation modules: context, camera, scene, dirty.
- [x] Create signex-renderer iced bridge skeleton (canvas clear and basic draw path).
- [x] Add theme slot infrastructure: StyleRef and palette uniform path.
- [x] Add clean-room module header template and commit checklist.

## Acceptance criteria

- [x] Application launches with shader canvas.
- [x] Scene upload and camera upload paths compile and run.
- [x] Theme slot pipeline is available, even if minimal.

## Required evidence notes

Create one note per checklist item under logs/ using the template.

Suggested filenames:

- logs/phase-0-task-01-gfx-foundation.md
- logs/phase-0-task-02-iced-bridge.md
- logs/phase-0-task-03-theme-slots.md
- logs/phase-0-task-04-clean-room-headers.md

Each note must include:

- Source
- Derivation
- Rationale
- Clean-room check
- Verification

## Non-goals

- No PCB 2D runtime implementation.
- No PCB 3D runtime implementation.
- No foreign parser or external GPL-derived code.
