pub mod lexer;
pub mod parser;

use std::path::PathBuf;

use crate::error::ModelImportError;
pub use parser::VrmlMesh;

/// Parse a VRML97 source file and return the flat mesh list.
pub fn load(path: &PathBuf) -> Result<Vec<VrmlMesh>, ModelImportError> {
    let source = std::fs::read_to_string(path).map_err(|e| ModelImportError::IoFailed {
        path: path.clone(),
        message: e.to_string(),
    })?;

    let (tokens, lines) = lexer::tokenize(&source);

    parser::parse(&tokens, &lines).map_err(|e| match e {
        parser::ParseError::UnexpectedEof { line } => ModelImportError::VrmlParseFailed {
            path: path.clone(),
            line,
            reason: "unexpected end of file".to_owned(),
        },
        parser::ParseError::UnresolvedUse { name } => {
            ModelImportError::VrmlUnresolvedUse { name }
        }
        parser::ParseError::MalformedNumber { line, value } => {
            ModelImportError::VrmlParseFailed {
                path: path.clone(),
                line,
                reason: format!("malformed number: {value}"),
            }
        }
    })
}
