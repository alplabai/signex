# Phase Note

## Metadata

- Phase: Milestone C (Preparation)
- Task ID: 03
- Task name: conversion and cache policy
- Owner: renderer-team
- Date: 2026-05-05
- Status: done

## Scope

Define deterministic conversion outputs, cache-key policy, invalidation behavior, and fallback rules for Milestone C import pipeline.

## Implementation notes

- Source intake policy documented:
  - Accepted import inputs: `.step`, `.stp`, `.wrl`, `.gltf`, `.glb`.
  - Runtime renderer input remains `.glb` only.
- Conversion output policy documented:
  - Import pipeline normalizes all supported source formats to GLB artifact for runtime.
  - Conversion metadata envelope includes source path, source mtime, converter version, and output GLB path.
- Cache-key policy documented:
  - Primary key: absolute source path + source mtime.
  - Secondary discriminator: converter version string (prevents stale re-use after converter behavior changes).
- Cache invalidation policy documented:
  - Rebuild on source mtime change.
  - Rebuild on converter version mismatch.
  - Rebuild on missing or unreadable cached GLB.
- Fallback behavior documented:
  - On conversion failure, keep last-known-good cache entry if source and converter version still match.
  - If no valid cache exists, surface import diagnostic and skip model attach for that asset.

## Determinism and parity notes

- Deterministic cache lookups rely on stable absolute paths and monotonic file timestamps.
- Import diagnostics are emitted in import stage; runtime renderer does not attempt source-format parsing.
- Runtime parity target for this task: identical GLB payload should produce identical cache-hit behavior under unchanged key tuple.

## Clean-room evidence

- Source: Renderer plan Section 9.2 (import pipeline definition and cache key).
- Derivation: direct policy expansion from `source path + mtime` baseline.
- Rationale: avoid ambiguous cache behavior and runtime format leakage.
- Clean-room check: No GPL-licensed source consulted
- Verification: milestone C issue/checklist updated with matching import/cache policy scope.

## Artifacts

- PR/commit: pending
- Test output: documentation-only task
- Screenshot/benchmark: n/a

## Exit checklist

- [x] Implementation completed
- [x] Source and derivation documented
- [x] Clean-room check confirmed
- [x] Verification artifact added
- [x] Linked from issue/checklist
