pub mod wrap;

use std::path::PathBuf;

use crate::error::ModelImportError;

pub use wrap::GltfWrapResult;

/// Load a `.gltf` JSON source and wrap it into a GLB payload.
pub fn load(path: &PathBuf, converter_version: &str) -> Result<GltfWrapResult, ModelImportError> {
    let source = std::fs::read_to_string(path).map_err(|e| ModelImportError::IoFailed {
        path: path.clone(),
        message: e.to_string(),
    })?;

    wrap::wrap_gltf(&source, path, converter_version)
}
