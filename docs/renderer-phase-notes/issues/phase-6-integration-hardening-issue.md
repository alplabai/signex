# Issue: Phase 6 - Integration and Hardening

Status: done

## Goal

Harden renderer integration paths, incremental update behavior, and regression confidence.

## Scope

- Wire all dirty-flag paths for partial uploads.
- Add viewport culling path for large schematic sheets.
- Add theme dirty path with uniform-only updates.
- Add end-to-end regression and golden render suite.

## Checklist

- [x] Wire dirty-flag-driven partial upload gating.
- [x] Integrate primitive culling for schematic viewports.
- [x] Implement theme dirty path without geometry rebuild.
- [x] Add deterministic regression and golden render suite.

## Acceptance criteria

- [x] Incremental updates avoid unnecessary uploads.
- [x] Large-sheet rendering remains stable with culling enabled.
- [x] Theme changes apply without geometry regeneration.
- [x] Regression and golden suite passes in CI.

## Required evidence notes

Suggested filenames:

- logs/phase-6-task-01-dirty-upload-gating.md
- logs/phase-6-task-02-culling-rstar-integration.md
- logs/phase-6-task-03-theme-dirty-path.md
- logs/phase-6-task-04-regression-golden-suite.md

## Non-goals

- No PCB 2D runtime implementation.
- No PCB 3D runtime implementation.
- No foreign parser or external GPL-derived code.
