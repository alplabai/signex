# Phase Note

## Metadata

- Phase: Milestone D (Preparation)
- Task ID: 06
- Task name: crate scaffold, error model, and test harness design
- Owner: renderer-team
- Date: 2026-05-05
- Status: done

## Scope

Design the `signex-model-import` crate structure, public API surface, error
model, and test harness plan for the Milestone D execution sprint.

## Crate scaffold

### Location

`crates/signex-model-import/`

### Cargo.toml dependencies

```toml
[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }   # if workspace already has it
base64 = "0.22"                    # GLTF embedded buffer encoding

[dev-dependencies]
tempfile = "3"
```

No `signex-gfx` or `signex-renderer` dependency. The crate is self-contained
and only produces GLB bytes / path artifacts.

### Module layout

```
src/
  lib.rs          — public API, re-exports
  error.rs        — ModelImportError, ImportWarning
  cache.rs        — CacheKey, CacheLookup, cache_path()
  vrml/
    mod.rs
    lexer.rs      — VRML tokenizer
    parser.rs     — recursive descent scene graph parser
    mesh.rs       — IndexedFaceSet → triangle mesh
  step/
    mod.rs
    p21.rs        — ISO 10303-21 entity tokenizer
    entities.rs   — entity map builder
    tess.rs       — plane-face tessellation (Phase A)
    subprocess.rs — external tool subprocess fallback (Phase B)
  gltf/
    mod.rs
    wrap.rs       — GLTF JSON + external resources → GLB packer
  normalize/
    mod.rs
    coord.rs      — coordinate system and unit transforms
    mesh.rs       — vertex dedup, indexing, primitive mode normalization
    scene.rs      — flat scene graph builder
  glb/
    mod.rs
    writer.rs     — GLB binary container serializer
```

## Public API

```rust
/// Synchronous entry point for converting a source model to a cached GLB.
pub fn import_model(request: ModelImportRequest) -> Result<ModelImportResult, ModelImportError>;

/// Async entry point (wraps import_model in a blocking task).
pub async fn import_model_async(request: ModelImportRequest) -> Result<ModelImportResult, ModelImportError>;

pub struct ModelImportRequest {
    pub model_id: String,
    pub source_path: PathBuf,
    pub cache_dir: PathBuf,
    pub converter_version: &'static str,
}

pub struct ModelImportResult {
    pub glb_path: PathBuf,
    pub cache_hit: bool,
    pub warnings: Vec<ImportWarning>,
    pub metadata: ImportMetadata,
}

pub struct ImportMetadata {
    pub source_format: SourceFormat,
    pub source_path: PathBuf,
    pub source_mtime: SystemTime,
    pub converter_version: String,
    pub mesh_count: usize,
    pub primitive_count: usize,
    pub byte_len: usize,
}

pub enum SourceFormat { Step, Vrml, Gltf, Glb }
```

## Error model

```rust
#[derive(Debug, thiserror::Error)]
pub enum ModelImportError {
    #[error("unsupported source format: {extension}")]
    UnsupportedFormat { extension: String },

    #[error("source file not found: {path:?}")]
    SourceNotFound { path: PathBuf },

    #[error("I/O error reading {path:?}: {message}")]
    IoFailed { path: PathBuf, message: String },

    #[error("STEP parse error at line {line}: {reason}")]
    StepParseFailed { path: PathBuf, line: usize, reason: String },

    #[error("VRML parse error at line {line}: {reason}")]
    VrmlParseFailed { path: PathBuf, line: usize, reason: String },

    #[error("GLTF parse error: {reason}")]
    GltfParseFailed { path: PathBuf, reason: String },

    #[error("tessellation failed for entity {entity_id}: {reason}")]
    TessellationFailed { entity_id: String, reason: String },

    #[error("GLB serialization failed: {reason}")]
    GlbWriteFailed { reason: String },

    #[error("cache directory error: {reason}")]
    CacheFailed { reason: String },
}

#[derive(Debug)]
pub enum ImportWarning {
    TextureMissing { uri: String },
    EmptyPrimitive { mesh_index: usize, primitive_index: usize },
    UnsupportedGeometry { entity_type: String },
    UnsupportedGltfExtension { name: String },
}
```

## Cache contract

Cache key = `(absolute_source_path, source_mtime_unix_sec, converter_version)`.

Cache path = `{cache_dir}/{sha256(cache_key_tuple)}.glb`.

On cache hit: return existing GLB path, set `cache_hit = true`.

On cache miss or invalidation: run conversion, write GLB to cache path, return.

## Test harness plan

### Fixture classes

| Class | Description | Format |
|-------|-------------|--------|
| Tier 0 | Synthetic minimal: single triangle | VRML, GLTF |
| Tier 1 | Single-body component (flat faces only) | VRML, STEP |
| Tier 2 | Multi-body component (IC + pins) | VRML |
| Tier 3 | GLTF with external .bin buffer | GLTF |
| Tier 4 | GLTF with embedded base64 buffers | GLTF |

Tier 0 and Tier 1 fixtures are generated synthetically in test code (no
binary blobs committed to the repository). Tier 2+ may use small real-world
samples if license-compatible (e.g., KiCad project library files under CC-BY-SA
4.0 or equivalent).

### Test categories

1. **Unit tests** (in-module): tokenizer correctness, entity map parsing,
   normalization transforms, GLB serializer round-trip.
2. **Integration tests** (in `tests/`): `import_model()` end-to-end for each
   fixture tier, cache hit/miss behavior, warning emission.
3. **Regression smoke**: synthetic fixture import produces stable GLB byte
   count across converter versions.

## Clean-room evidence

- Source: glTF 2.0 spec (crate structure and API design), Milestone C prep
  contracts (error model boundary).
- Derivation: crate layout follows single-responsibility principle; error model
  covers all failure modes identified in Tasks 02–04.
- Rationale: self-contained crate with no runtime renderer dependency ensures
  the import pipeline can be developed, tested, and versioned independently.
- Clean-room check: No GPL-licensed source consulted.
- Verification: Milestone D issue Task 06 marked done; checklist updated.

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
