# Phase Note

## Metadata

- Phase: Milestone D (Preparation)
- Task ID: 05
- Task name: GLB output normalization and coordinate system contract
- Owner: renderer-team
- Date: 2026-05-05
- Status: done

## Scope

Define the normalization rules applied to all GLB artifacts produced by
`signex-model-import`, regardless of source format. These rules ensure that
the runtime (`signex-renderer`) receives a consistent, predictable GLB
payload.

## Coordinate system

| Property | Rule |
|----------|------|
| Handedness | Right-handed (glTF 2.0 spec baseline) |
| Up axis | Y-up (glTF 2.0 spec baseline) |
| STEP source | May be Z-up; rotate −90° around X axis during tessellation export |
| VRML source | Already Y-up right-handed; no rotation needed |
| GLTF source | Already Y-up right-handed; no rotation needed |

The rotation from Z-up to Y-up for STEP outputs is applied at the root node
level by inserting a `transform` matrix on the scene root node, not by
modifying vertex data. This preserves the original tessellation geometry.

## Unit normalization

glTF 2.0 does not mandate a unit system, but by convention and KiCad export
behavior:

| Source | Native unit | Target unit | Conversion factor |
|--------|-------------|-------------|-------------------|
| STEP | mm | mm | 1.0 (no change) |
| VRML (KiCad) | mm | mm | 1.0 (no change) |
| GLTF (pass-through) | as authored | as authored | preserved |

All `signex-model-import` GLB outputs use millimetres as the working unit.
The runtime renders in mm-space (consistent with PCB canvas coordinate space).

If a GLTF source uses metres (indicated by `asset.extras.unit` or convention),
apply a scale of 1000.0 via the root node transform.

## Mesh normalization rules

1. **Vertex deduplication:** deduplicate vertices with identical position
   within the same mesh primitive (ε = 1e-6 mm). This reduces GLB size and
   improves rendering performance for dense tessellations.

2. **Index buffer:** all mesh primitives must use indexed geometry
   (`mesh.primitives[i].indices` must be present). Generate indices for any
   unindexed geometry.

3. **Position attribute:** `POSITION` attribute required on all primitives.
   Type: `FLOAT`, component: `VEC3`.

4. **Normal attribute:** `NORMAL` attribute is optional for Milestone D execution.
   If absent, the runtime uses face normals computed in the shader.

5. **Color attribute:** if source provides per-mesh color (VRML `diffuseColor`,
   GLTF material), map to a glTF `materials[i]` entry with `pbrMetallicRoughness
   .baseColorFactor`. Do not embed colors as vertex attributes.

6. **Primitive mode:** use `TRIANGLES` (mode 4) for all output primitives. If
   source uses quads, triangulate via fan split before output.

7. **Empty primitives:** primitives with zero vertices or zero indices must be
   omitted from the output mesh; log as `ImportWarning::EmptyPrimitive`.

## Scene graph structure

Normalized output GLB uses a flat scene graph:

```
scene 0
  └── node 0  (root transform: coordinate system + unit correction)
        ├── node 1 → mesh 0  (first component body)
        ├── node 2 → mesh 1
        └── ...
```

Multiple component bodies (e.g., IC body, pins, silkscreen) become separate
nodes under the single root transform node.

## Asset metadata

The normalized GLB `asset` field must contain:

```json
{
  "asset": {
    "version": "2.0",
    "generator": "signex-model-import",
    "extras": {
      "source_format": "step" | "vrml" | "gltf",
      "source_path": "<absolute path>",
      "source_mtime": "<ISO 8601 timestamp>",
      "converter_version": "<semver string>"
    }
  }
}
```

This metadata is the basis for the cache-key validation described in
Milestone C prep Task 03.

## Clean-room sources

- glTF 2.0 specification (Khronos Group, Apache 2.0).
- ISO 10303-21 coordinate conventions (public standard).
- ISO/IEC 14772-1:1997 VRML97 coordinate conventions (public standard).

## Clean-room evidence

- Source: glTF 2.0 spec, ISO 10303-21, ISO/IEC 14772-1:1997.
- Derivation: normalization rules derived directly from public specifications
  and from Milestone C runtime contract requirements.
- Rationale: deterministic normalization ensures the runtime receives
  consistent inputs regardless of source format path, eliminating
  format-specific branches in `signex-renderer`.
- Clean-room check: No GPL-licensed source consulted.
- Verification: Milestone D issue Task 05 marked done; checklist updated.

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
