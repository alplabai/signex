# Issue: Phase 5 - Grid and Overlay System

Status: done

## Goal

Implement grid and overlay rendering passes for schematic interaction visuals.

## Scope

- Implement crosshair grid shader with LOD-aware behavior.
- Implement overlay emitters for preview, ghost, lasso, and snap visuals.
- Implement ERC marker rendering with severity-based styling tokens.
- Validate overlay ordering and toggling behavior.

## Checklist

- [x] Implement grid shader and LOD fade rules.
- [x] Integrate overlay emitter set (preview, ghost, lasso, snap).
- [x] Add ERC marker primitives and severity style mapping.
- [x] Validate overlay pass ordering and toggling.

## Acceptance criteria

- [x] Grid is density-aware across zoom levels.
- [x] Overlay visuals render in the correct pass order.
- [x] ERC markers render with warning/error/info token styles.
- [x] Grid and overlay paths pass compile, smoke, and fixture validation.

## Required evidence notes

Suggested filenames:

- logs/phase-5-task-01-grid-lod-shader.md
- logs/phase-5-task-02-overlay-emitter-integration.md
- logs/phase-5-task-03-erc-marker-styling.md
- logs/phase-5-task-04-overlay-order-validation.md

## Non-goals

- No PCB 2D runtime implementation.
- No PCB 3D runtime implementation.
- No foreign parser or external GPL-derived code.
