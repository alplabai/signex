# Phase Note

## Metadata

- Phase: Milestone D (Preparation)
- Task ID: 02
- Task name: STEP/STP source format analysis and parser contract
- Owner: renderer-team
- Date: 2026-05-05
- Status: done

## Scope

Analyze the STEP/P21 physical file format as used by KiCad STEP exports and
define the clean-room parser contract for `signex-model-import`.

## Format overview

STEP files from KiCad use the ISO 10303-21 physical file format (commonly called
P21 or "STEP Physical File"). The container structure is public and not
proprietary. Key facts relevant to import:

- File starts with `ISO-10303-21;` header sentinel.
- Sections: `HEADER;` (file metadata) and `DATA;` (entity instances).
- Each data line is an entity instance: `#id = ENTITY_TYPE(params...);`
- KiCad uses AP214 and AP242 application protocols, which map to geometric
  entities: `ADVANCED_FACE`, `CLOSED_SHELL`, `MANIFOLD_SOLID_BREP`,
  `CYLINDRICAL_SURFACE`, `CONICAL_SURFACE`, `PLANE`, `B_SPLINE_SURFACE`, etc.
- Coordinates are in millimetres. Axes: X right, Y up, Z toward viewer (varies
  by exporter; must be detected from `AXIS2_PLACEMENT_3D`).

## Parser contract

### Parsing strategy

P21 is line-oriented and regular enough for a custom clean-room parser:

1. Read lines until `DATA;` sentinel.
2. For each `#id = TYPE(...)` line: tokenize into entity ID, type name, and
   parameter list.
3. Build an in-memory entity map: `HashMap<u32, Entity>`.
4. Resolve geometric entities into tessellated triangle meshes via a traversal
   starting at `PRODUCT_DEFINITION` → `SHAPE_REPRESENTATION` → mesh entities.

### Tessellation approach

B-rep tessellation (converting boundary representation solids to triangle meshes)
is required for STEP. Two implementation paths:

**Path A — Pure Rust tessellation (preferred for Milestone D exec):**
- Implement plane-face tessellation only (flat faces → triangles via fan
  triangulation of `ADVANCED_FACE` with `PLANE` geometry).
- Defer curved surfaces (cylindrical, conical, B-spline) to a later sprint.
- This covers a large fraction of real PCB component bodies (ICs, connectors,
  passives with flat top faces).

**Path B — Subprocess delegation (fallback):**
- Invoke FreeCAD headless (`FreeCADCmd`) or `stepconvert` as a subprocess.
- Capture GLB output from stdout/tempfile.
- Isolates GPL-licensed OCCT behind a process boundary (no Rust crate dependency).
- Document the subprocess interface contract: invocation flags, exit codes,
  output format, timeout.

### Output

Tessellated mesh → GLB, normalized per Task 05 contract.

### Error model (STEP-specific variants)

| Variant | Condition |
|---------|-----------|
| `ParseFailed { path, line, reason }` | P21 syntax error |
| `UnsupportedGeometry { entity_type }` | Entity type not yet implemented |
| `TessellationFailed { entity_id, reason }` | Mesh generation failed |
| `EmptyGeometry { path }` | No mesh-bearing entities found |

## KiCad STEP conventions

- KiCad STEP exports place the board in the XY plane with Z=0 at board bottom.
- Component models are positioned via `PRODUCT_DEFINITION_PLACEMENT`.
- Color is typically set via `STYLED_ITEM` / `PRESENTATION_STYLE_ASSIGNMENT`.
  For Milestone D execution, colors can be approximated or ignored (grey fallback).

## Clean-room sources

- ISO 10303-1:1994 (Overview and fundamental principles) — public standard.
- ISO 10303-21:2016 (Physical file format) — public standard.
- IFC file format uses the same P21 physical encoding; IFC open standard
  documentation is publicly available and serves as a cross-reference.
- No KiCad source code consulted.
- No GPL-licensed OCCT source consulted.

## Clean-room evidence

- Source: ISO 10303-21 public standard, IFC open standard.
- Derivation: parser contract derived from public format specification only.
- Rationale: isolate STEP parsing complexity from runtime; support incremental
  surface coverage without blocking the vertical slice on curved geometry.
- Clean-room check: No GPL-licensed source consulted.
- Verification: Milestone D issue Task 02 marked done; checklist updated.

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
