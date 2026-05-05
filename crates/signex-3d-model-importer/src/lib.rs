pub mod cache;
pub mod error;
pub mod gltf;
pub mod glb;
pub mod normalize;
pub mod step;
pub mod vrml;

use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub use error::{ImportWarning, ModelImportError};

/// Supported source model formats.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceFormat {
    Vrml,
    Step,
    Gltf,
    Glb,
}

/// Request parameters for a model import operation.
pub struct ModelImportRequest {
    /// Unique identifier for this model (used for diagnostics).
    pub model_id: String,
    /// Absolute path to the source model file.
    pub source_path: PathBuf,
    /// Directory for cached GLB outputs.
    pub cache_dir: PathBuf,
    /// Semver string for this converter build (affects cache key).
    pub converter_version: &'static str,
}

/// Result of a successful model import.
pub struct ModelImportResult {
    /// Path to the produced (or cached) GLB file.
    pub glb_path: PathBuf,
    /// `true` if the GLB was served from cache without re-converting.
    pub cache_hit: bool,
    /// Non-fatal warnings collected during import.
    pub warnings: Vec<ImportWarning>,
    /// Metadata about the imported model.
    pub metadata: ImportMetadata,
}

/// Metadata embedded in the GLB asset.extras and returned to the caller.
pub struct ImportMetadata {
    pub source_format: SourceFormat,
    pub source_path: PathBuf,
    pub source_mtime: SystemTime,
    pub converter_version: String,
    pub mesh_count: usize,
    pub primitive_count: usize,
    pub byte_len: usize,
}

/// Synchronous entry point: convert `request.source_path` → cached GLB.
///
/// On cache hit the existing GLB is returned without re-converting.
pub fn import_model(request: ModelImportRequest) -> Result<ModelImportResult, ModelImportError> {
    let source_path = &request.source_path;

    if !source_path.exists() {
        return Err(ModelImportError::SourceNotFound { path: source_path.clone() });
    }

    let format = detect_format(source_path)?;

    let source_mtime = source_path
        .metadata()
        .and_then(|m| m.modified())
        .map_err(|e| ModelImportError::IoFailed {
            path: source_path.clone(),
            message: e.to_string(),
        })?;

    let glb_path = cache::cache_path(
        &request.cache_dir,
        source_path,
        source_mtime,
        request.converter_version,
    )?;

    // Cache hit
    if cache::is_cache_valid(&glb_path) {
        let byte_len = glb_path.metadata().map(|m| m.len() as usize).unwrap_or(0);
        return Ok(ModelImportResult {
            glb_path,
            cache_hit: true,
            warnings: vec![],
            metadata: ImportMetadata {
                source_format: format,
                source_path: source_path.clone(),
                source_mtime,
                converter_version: request.converter_version.to_owned(),
                mesh_count: 0,   // not re-parsed on hit
                primitive_count: 0,
                byte_len,
            },
        });
    }

    // Cache miss: convert
    let (glb_bytes, warnings, mesh_count, primitive_count) = match format {
        SourceFormat::Vrml => {
            let meshes = vrml::load(source_path)?;
            let mesh_count = meshes.len();
            let (json, bin) = normalize::meshes_to_gltf(
                &meshes,
                "vrml",
                &source_path.to_string_lossy(),
                request.converter_version,
            );
            let glb = glb::writer::write_glb(&json, &bin)?;
            (glb, vec![], mesh_count, mesh_count)
        }
        SourceFormat::Step => {
            let result = step::load(source_path)?;
            let mesh_count = result.meshes.len();
            let mut warnings = Vec::new();
            for entity in result.unsupported_entities {
                warnings.push(ImportWarning::UnsupportedGeometry { entity_type: entity });
            }
            let (json, bin) = normalize::meshes_to_gltf(
                &result.meshes,
                "step",
                &source_path.to_string_lossy(),
                request.converter_version,
            );
            let glb = glb::writer::write_glb(&json, &bin)?;
            (glb, warnings, mesh_count, mesh_count)
        }
        SourceFormat::Gltf => {
            let wrapped = gltf::load(source_path, request.converter_version)?;
            let glb = glb::writer::write_glb(&wrapped.json_bytes, &wrapped.bin_bytes)?;
            (
                glb,
                wrapped.warnings,
                wrapped.mesh_count,
                wrapped.primitive_count,
            )
        }
        SourceFormat::Glb => {
            // GLB pass-through: copy as-is
            let bytes = std::fs::read(source_path).map_err(|e| ModelImportError::IoFailed {
                path: source_path.clone(),
                message: e.to_string(),
            })?;
            (bytes, vec![], 0, 0)
        }
    };

    // Write to cache
    std::fs::create_dir_all(&request.cache_dir).map_err(|e| ModelImportError::CacheFailed {
        reason: e.to_string(),
    })?;
    std::fs::write(&glb_path, &glb_bytes).map_err(|e| ModelImportError::CacheFailed {
        reason: e.to_string(),
    })?;

    let byte_len = glb_bytes.len();

    Ok(ModelImportResult {
        glb_path,
        cache_hit: false,
        warnings,
        metadata: ImportMetadata {
            source_format: format,
            source_path: source_path.clone(),
            source_mtime,
            converter_version: request.converter_version.to_owned(),
            mesh_count,
            primitive_count,
            byte_len,
        },
    })
}

fn detect_format(path: &Path) -> Result<SourceFormat, ModelImportError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "wrl" | "vrml" => Ok(SourceFormat::Vrml),
        "stp" | "step" => Ok(SourceFormat::Step),
        "gltf" => Ok(SourceFormat::Gltf),
        "glb" => Ok(SourceFormat::Glb),
        _ => Err(ModelImportError::UnsupportedFormat { extension: ext }),
    }
}

