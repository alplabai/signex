use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Invalid S-expression: {0}")]
    InvalidSExpr(String),

    #[error("Missing field: {0}")]
    MissingField(String),

    #[error("Invalid value: {0}")]
    InvalidValue(String),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Unsupported version: {0}")]
    UnsupportedVersion(String),
}

impl From<String> for ParseError {
    fn from(s: String) -> Self {
        ParseError::InvalidSExpr(s)
    }
}
