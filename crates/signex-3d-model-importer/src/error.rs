use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum ModelImportError {
    #[error("unsupported source format: {extension}")]
    UnsupportedFormat { extension: String },

    #[error("source file not found: {path:?}")]
    SourceNotFound { path: PathBuf },

    #[error("I/O error reading {path:?}: {message}")]
    IoFailed { path: PathBuf, message: String },

    #[error("STEP parse error at line {line} in {path:?}: {reason}")]
    StepParseFailed { path: PathBuf, line: usize, reason: String },

    #[error("VRML parse error at line {line} in {path:?}: {reason}")]
    VrmlParseFailed { path: PathBuf, line: usize, reason: String },

    #[error("unresolved VRML USE node: {name}")]
    VrmlUnresolvedUse { name: String },

    #[error("GLTF parse error in {path:?}: {reason}")]
    GltfParseFailed { path: PathBuf, reason: String },

    #[error("tessellation failed for entity {entity_id}: {reason}")]
    TessellationFailed { entity_id: String, reason: String },

    #[error("GLB serialization failed: {reason}")]
    GlbWriteFailed { reason: String },

    #[error("cache directory error: {reason}")]
    CacheFailed { reason: String },
}

#[derive(Debug, Clone)]
pub enum ImportWarning {
    TextureMissing { uri: String },
    EmptyPrimitive { mesh_index: usize, primitive_index: usize },
    UnsupportedGeometry { entity_type: String },
    UnsupportedGltfExtension { name: String },
}
