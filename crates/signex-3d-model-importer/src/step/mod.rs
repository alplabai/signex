pub mod p21;

use std::path::PathBuf;

use crate::error::ModelImportError;
pub use p21::StepMeshResult;

/// Parse a STEP/P21 source file and convert supported planar faces to meshes.
pub fn load(path: &PathBuf) -> Result<StepMeshResult, ModelImportError> {
    let source = std::fs::read_to_string(path).map_err(|e| ModelImportError::IoFailed {
        path: path.clone(),
        message: e.to_string(),
    })?;

    p21::parse_to_meshes(&source).map_err(|err| match err {
        p21::ParseError::DataSectionMissing => ModelImportError::StepParseFailed {
            path: path.clone(),
            line: 1,
            reason: "missing DATA section".to_owned(),
        },
        p21::ParseError::MalformedEntity { line, reason } => ModelImportError::StepParseFailed {
            path: path.clone(),
            line,
            reason,
        },
        p21::ParseError::MalformedNumber { line, value } => ModelImportError::StepParseFailed {
            path: path.clone(),
            line,
            reason: format!("malformed number: {value}"),
        },
        p21::ParseError::EmptyGeometry => ModelImportError::StepParseFailed {
            path: path.clone(),
            line: 1,
            reason: "no tessellatable faces found".to_owned(),
        },
    })
}
