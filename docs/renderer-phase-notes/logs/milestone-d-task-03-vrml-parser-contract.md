# Phase Note

## Metadata

- Phase: Milestone D (Preparation)
- Task ID: 03
- Task name: VRML/WRL source format analysis and parser contract
- Owner: renderer-team
- Date: 2026-05-05
- Status: done

## Scope

Analyze the VRML 2.0 / VRML97 format as used by KiCad WRL model exports and
define the clean-room parser contract for `signex-model-import`.

## Format overview

KiCad uses VRML 2.0 (also known as VRML97, ISO/IEC 14772-1:1997) for 3D
component models. Unlike STEP, VRML already contains tessellated geometry —
no B-rep tessellation step is required. Key facts:

- File starts with: `#VRML V2.0 utf8`
- Root is a scene graph of `Transform`, `Shape`, `Group`, and `DEF`/`USE` nodes.
- Geometry is expressed as `IndexedFaceSet`: a vertex coordinate array plus
  index arrays that reference them.
- Materials are defined via `Appearance` nodes containing `Material` with
  `diffuseColor`, `specularColor`, `emissiveColor`, `transparency` fields.
- Coordinate system: VRML is Y-up, right-handed (same as glTF). No coordinate
  flip needed for glTF output.
- Units: KiCad WRL files use millimetres (KiCad convention). glTF spec uses
  metres; normalization is required — see Task 05.

## Parser contract

### Parsing strategy

VRML is a recursive text format. A clean-room parser can be implemented with a
straightforward recursive descent approach:

1. Tokenize the file (whitespace-separated tokens, `{` `}` `[` `]` delimiters,
   `#` comments stripped to end of line).
2. Parse the scene graph recursively: recognize `DEF name NodeType { ... }`,
   `USE name`, and bare `NodeType { ... }`.
3. Accumulate `Transform` matrices along the traversal stack.
4. At each `Shape { geometry IndexedFaceSet { ... } }` node: extract vertex
   coordinates, apply accumulated transform, push to mesh buffer.
5. Extract `Appearance.Material` diffuse color as mesh tint.

### DEF/USE handling

`DEF foo Transform { ... }` defines a named node; `USE foo` references it. The
parser must maintain a `HashMap<String, SceneNode>` for DEF lookups and clone
the sub-tree on USE, applying the current stack transform.

### Output

Parsed meshes with per-mesh tint color → GLB, normalized per Task 05.

### Error model (VRML-specific variants)

| Variant | Condition |
|---------|-----------|
| `ParseFailed { path, line, reason }` | VRML syntax error |
| `UnresolvedUse { name }` | USE references undefined DEF |
| `EmptyGeometry { path }` | No IndexedFaceSet nodes found |
| `MalformedIndexedFaceSet { node_id }` | coord/index array length mismatch |

## KiCad WRL conventions

- KiCad WRL models are placed at origin with scale 1:1 in mm.
- A root `Transform` node typically applies position and orientation for the
  component footprint mounting point.
- Material colors in KiCad WRL are typically coarse approximations (e.g., grey
  for IC bodies, gold for pads). Preserving them as-is is acceptable.
- `creaseAngle` on `IndexedFaceSet` affects normal smoothing — for Milestone D
  execution, per-face flat normals are acceptable.

## Advantages over STEP for first vertical slice

VRML is the recommended first implementation target because:

1. Already tessellated — no B-rep algorithm needed.
2. Simple recursive text format — parser is ~400 lines of Rust.
3. Color information is explicit and straightforward to map.
4. Covers all KiCad community-library component models (`.wrl`).

## Clean-room sources

- ISO/IEC 14772-1:1997 (VRML97 specification) — public international standard.
- VRML97 specification is freely available at web3d.org as the successor
  consortium to the original ISO working group.
- No KiCad source code consulted.

## Clean-room evidence

- Source: ISO/IEC 14772-1:1997 (VRML97), web3d.org.
- Derivation: parser contract derived from public format specification only.
- Rationale: VRML/WRL is the simplest supported format (pre-tessellated,
  straightforward text), making it the lowest-risk first vertical slice target.
- Clean-room check: No GPL-licensed source consulted.
- Verification: Milestone D issue Task 03 marked done; checklist updated.

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
