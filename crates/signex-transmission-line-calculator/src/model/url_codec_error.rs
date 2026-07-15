use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq)]
pub enum UrlCodecError {
    #[error("missing query parameter: {name}")]
    MissingParameter { name: String },
    #[error("invalid query parameter {name}: {value}")]
    InvalidParameter { name: String, value: String },
    #[error("invalid circuit token: {token}")]
    InvalidCircuitToken { token: String },
    #[error("unsupported S-parameter URL payload")]
    UnsupportedSParameterPayload,
    #[error("touchstone parse failed: {reason}")]
    TouchstoneParseFailed { reason: String },
}
