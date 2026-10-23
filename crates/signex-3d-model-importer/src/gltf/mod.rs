pub mod wrap;

use std::path::Path;

use crate::error::ModelImportError;

pub use wrap::GltfWrapResult;

/// Load a `.gltf` JSON source and wrap it into a GLB payload.
pub fn load(path: &Path, converter_version: &str) -> Result<GltfWrapResult, ModelImportError> {
    let source =
        crate::read::read_text_capped(path, crate::read::MAX_GLTF_JSON_BYTES).map_err(|e| {
            ModelImportError::IoFailed {
                path: path.to_path_buf(),
                message: e.to_string(),
            }
        })?;

    wrap::wrap_gltf(&source, path, converter_version)
}
