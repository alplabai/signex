pub mod lexer;
pub mod parser;

use std::path::Path;

use crate::error::ModelImportError;
pub use parser::{ParseWarning, VrmlMesh};

/// Parse a VRML97 source file and return the flat mesh list, plus the
/// recoverable findings that changed the geometry on the way (#805).
pub fn load(path: &Path) -> Result<(Vec<VrmlMesh>, Vec<ParseWarning>), ModelImportError> {
    let source = crate::read::read_text_capped(path, crate::read::MAX_READ_BYTES).map_err(|e| {
        ModelImportError::IoFailed {
            path: path.to_path_buf(),
            message: e.to_string(),
        }
    })?;

    let (tokens, lines) = lexer::tokenize(&source);

    parser::parse(&tokens, &lines).map_err(|e| match e {
        parser::ParseError::UnexpectedEof { line } => ModelImportError::VrmlParseFailed {
            path: path.to_path_buf(),
            line,
            reason: "unexpected end of file".to_owned(),
        },
        parser::ParseError::NestingTooDeep { line, limit } => ModelImportError::VrmlParseFailed {
            path: path.to_path_buf(),
            line,
            reason: format!("node nesting deeper than {limit}"),
        },
        parser::ParseError::MalformedNumber { line, value } => ModelImportError::VrmlParseFailed {
            path: path.to_path_buf(),
            line,
            reason: format!("malformed number: {value}"),
        },
    })
}
