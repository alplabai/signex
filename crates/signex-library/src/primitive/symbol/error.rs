//! Errors of the `.snxsym` reader and writer.

use super::SymbolLoadRepair;

/// `U+000D` for a message; takes the reference the error fields give.
fn code_point(c: &char) -> String {
    crate::toml_literal::code_point(*c)
}

/// Error variants for [`super::SymbolFile`] parsers + serialisers.
#[derive(Debug, thiserror::Error)]
pub enum SymbolFileError {
    #[error("empty .snxsym file")]
    Empty,
    #[error("invalid UTF-8 in TOML payload: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("TOML deserialise failed: {0}")]
    TomlDeserialize(#[from] toml::de::Error),
    #[error("non-finite number (nan/inf) at `{path}`; fix the value to open this file")]
    NonFiniteNumber { path: String },
    #[error("TOML serialise failed: {0}")]
    TomlSerialize(toml::ser::Error),
    #[error(".snxsym {0}")]
    UnsupportedFormat(#[from] signex_types::format_version::FormatVersionError),
    #[error(
        "TSV cell in column {column:?} contains the triple-quote: {value:?}; \
         cells must be free of the literal \"'''\""
    )]
    InvalidTsvCell { column: &'static str, value: String },
    /// #3129: a pin cell holds a control character the pin table cannot
    /// store (a literal string allows none but tab, and tab separates cells).
    /// `symbol` is empty until `SymbolFile::to_toml_string` names it.
    #[error(
        "symbol {symbol:?}, pin {pin:?}: the {column} holds the control character {}, \
         which a .snxsym cannot store; remove it to save",
        code_point(.character)
    )]
    ControlCharInPinCell {
        symbol: String,
        pin: String,
        column: &'static str,
        character: char,
    },
    /// #3129: the file on disk holds such a character already, so a strict
    /// read cannot open it; `SymbolFile::from_bytes_repairing` can.
    #[error("{first}; open the file in the Symbol Editor to repair it ({count} in all)")]
    RepairableControlChars {
        first: SymbolLoadRepair,
        count: usize,
    },
    /// A pin function name containing `|`, which separates a pin's
    /// functions on disk, so it would reload as two functions.
    #[error("pin function {value:?} contains '|', which separates pin functions; rename it")]
    PipeInPinFunction { value: String },
    #[error("pins_tsv block is empty (no header row)")]
    EmptyPinsTsv,
    #[error("pins_tsv header does not match the expected schema; got columns {got:?}")]
    PinsTsvSchemaMismatch { got: Vec<String> },
    #[error("pins_tsv row {row_index} has {got} cells; header declares {expected}")]
    PinsTsvCellCountMismatch {
        row_index: usize,
        got: usize,
        expected: usize,
    },
    #[error("unknown {kind} token {got:?} in pins_tsv cell")]
    UnknownEnumToken { kind: &'static str, got: String },
    #[error("invalid numeric cell in column {column:?}: {value:?}")]
    InvalidNumericCell { column: &'static str, value: String },
    /// #828: a NaN or infinite number cannot be written to a `.snxsym`.
    /// `field` names where it sits (a pin cell or a key path), so the
    /// save fails loudly instead of writing an empty cell or panicking.
    #[error("{field} holds a non-finite number (NaN or infinity); it cannot be saved")]
    NonFiniteField { field: String },
    #[error(
        "invalid boolean cell in column {column:?}: {value:?} (expected \"true\" or \"false\")"
    )]
    InvalidBoolCell { column: &'static str, value: String },
    /// MD-27: pin TSV payload contains the placeholder sentinel that
    /// `to_toml_string` uses for the post-emit `String::replace` pass.
    /// Mirrors `SimFileError::InvalidBody` — output corruption guard.
    #[error("symbol {symbol_index} pin TSV contains the placeholder sentinel")]
    InvalidPinsTsv { symbol_index: usize },
}

impl From<signex_types::numeric::TomlFiniteError> for SymbolFileError {
    fn from(e: signex_types::numeric::TomlFiniteError) -> Self {
        use signex_types::numeric::TomlFiniteError as E;
        match e {
            E::Toml(e) => Self::TomlDeserialize(e),
            E::NonFinite { path } => Self::NonFiniteNumber { path },
        }
    }
}

impl SymbolFileError {
    /// Name the symbol a pin-cell error came from.
    pub(super) fn in_symbol(self, name: &str) -> Self {
        match self {
            Self::ControlCharInPinCell {
                pin,
                column,
                character,
                ..
            } => Self::ControlCharInPinCell {
                symbol: name.to_string(),
                pin,
                column,
                character,
            },
            other => other,
        }
    }
}
