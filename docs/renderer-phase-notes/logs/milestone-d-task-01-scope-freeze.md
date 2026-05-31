# Phase Note

## Metadata

- Phase: Milestone D (Preparation)
- Task ID: 01
- Task name: scope freeze and dependency boundaries
- Owner: renderer-team
- Date: 2026-05-05
- Status: done

## Scope

Freeze the exact Milestone D preparation scope so the `signex-model-import`
implementation sprint starts without ambiguity.

## Implementation notes

- Locked Milestone D scope to: STEP/STP, VRML/WRL, GLTF source format analysis
  and import pipeline design for `signex-model-import` crate.
- Locked runtime boundary: `signex-renderer` continues to accept GLB only. No
  format-specific parsing is introduced in the runtime crate. This boundary was
  established in Milestone C and is not reopened here.
- Locked crate ownership:
  - `signex-model-import`: source parsing, tessellation (where needed), GLB
    normalization, cache key management, import diagnostics.
  - `signex-renderer` / `signex-gfx`: GLB consumption and draw-pass integration only.
- Locked conversion trigger model: import is triggered programmatically (not via
  GUI wizard). The UI layer calls a stable async API exposed by `signex-model-import`.
- Locked dependency constraints:
  - No OCCT (Open CASCADE Technology) dependency in `signex-model-import` or any
    runtime crate. Tessellation must use clean-room Rust implementations or
    subprocess delegation to an external tool (with documented interface contract).
  - No GPL-licensed geometry kernel in the crate graph.
  - `serde_json` and `base64` are acceptable for GLTF → GLB wrapping.
- Non-goals confirmed:
  - No mesh simplification or LOD in this milestone.
  - No GUI file picker or import wizard.
  - No cloud or network-based conversion.
  - No STEP/VRML parsing in `signex-renderer`.

## Dependency graph

```
KiCad project (source models)
       │
       ▼
signex-model-import
  ├── STEP/STP parser → tessellate → GLB
  ├── VRML/WRL parser → pack geometry → GLB
  ├── GLTF → GLB wrapper
  └── cache manager (source path + mtime + converter version)
       │
       ▼  (GLB path or bytes)
signex-renderer (runtime, GLB-only)
```

## Clean-room evidence

- Source: Milestone C prep Task 02 (import pipeline contract), Task 03 (cache
  policy), and renderer plan ROADMAP.md WS-3D.
- Derivation: direct scope expansion from Milestone C contracts, adding format
  analysis tasks for each supported source format.
- Rationale: prevents runtime/import boundary drift and keeps the crate graph
  free of GPL-licensed geometry kernels.
- Clean-room check: No GPL-licensed source consulted.
- Verification: Milestone D issue and checklist updated with matching scope and
  non-goals.

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
