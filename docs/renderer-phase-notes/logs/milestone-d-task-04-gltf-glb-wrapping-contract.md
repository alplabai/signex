# Phase Note

## Metadata

- Phase: Milestone D (Preparation)
- Task ID: 04
- Task name: GLTF → GLB container wrapping contract
- Owner: renderer-team
- Date: 2026-05-05
- Status: done

## Scope

Define the contract for converting a GLTF JSON file (with optional external
binary and texture resources) into a self-contained GLB container artifact.

## Format overview

glTF 2.0 exists in two forms:

- **GLTF** (JSON form): a `.gltf` JSON file plus optional `.bin` buffer files
  and texture image files (`.png`, `.jpg`) referenced by URI.
- **GLB** (binary form): a single binary container with a 12-byte header, a
  JSON chunk (type `0x4E4F534A`), and an optional binary chunk (type `0x004E4942`).

The runtime (`signex-renderer`) already consumes GLB exclusively. GLTF wrapping
is therefore a straightforward packaging operation.

## Wrapping algorithm

1. Read the `.gltf` JSON.
2. For each `buffers[i].uri` that references an external file: read the file,
   base64-decode if needed, concatenate into a single binary buffer.
3. Update `buffers[i].uri` fields to `null` (embedded buffer) and update
   `buffers[i].byteLength` to match the concatenated buffer.
4. For each `images[i].uri` that references an external file: read the file,
   embed as a base64 data URI in the JSON, or embed in the binary buffer and
   replace `uri` with a `bufferView` reference.
5. Serialize the updated JSON to UTF-8, pad to 4-byte alignment with spaces.
6. Pack the GLB container:
   - Header: magic `0x46546C67`, version `2`, total length.
   - JSON chunk: length + type `0x4E4F534A` + padded JSON bytes.
   - BIN chunk (if buffer is non-empty): length + type `0x004E4942` + buffer bytes.

## Notes on embedded textures

For Milestone D execution, only `.png` and `.jpeg` embedded textures are
required. Other image formats (`.ktx2`, `.webp`) are deferred.

If a texture file referenced in the GLTF cannot be found, emit an
`ImportWarning::TextureMissing` (non-fatal) and substitute a 1×1 white pixel
PNG as placeholder.

## Error model (GLTF-specific variants)

| Variant | Condition |
|---------|-----------|
| `ParseFailed { path, reason }` | JSON parse error |
| `ExternalResourceMissing { uri }` | Referenced `.bin` or image file not found (fatal) |
| `TextureMissing { uri }` | Referenced texture image not found (warning, substituted) |
| `UnsupportedGltfExtension { name }` | Required extension not implemented |

## Pass-through case

A `.gltf` file whose buffers are already embedded as data URIs (`data:application/
octet-stream;base64,...`) is wrapped without any external file I/O. This is the
fastest path and should be detected first.

## Clean-room sources

- glTF 2.0 specification (Khronos Group, Apache 2.0 licensed) — fully public.
- GLB container format defined in the same specification.

## Clean-room evidence

- Source: glTF 2.0 specification, Khronos Group (Apache 2.0).
- Derivation: wrapping algorithm follows binary container spec directly.
- Rationale: GLTF → GLB is a pure packaging step requiring no geometry processing;
  correctness is verifiable against the public spec.
- Clean-room check: No GPL-licensed source consulted.
- Verification: Milestone D issue Task 04 marked done; checklist updated.

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
