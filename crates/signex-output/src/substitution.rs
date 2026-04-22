//! Text substitution — resolves `${TITLE}`, `${DATE}`, `${REV}`, etc.
//!
//! See `OUTPUT_PLAN.md` §5. Resolved at render time only, never baked into
//! the KiCad file — `.kicad_sch` stores literal `${TITLE}` strings so the
//! round-trip stays lossless.

use thiserror::Error;

use crate::ProjectMetadata;

/// Binds tokens to their values for a specific render pass — one sheet, one
/// project snapshot. Constructed per-export in the app layer.
#[derive(Debug, Clone)]
pub struct SubstitutionContext<'a> {
    pub metadata: &'a ProjectMetadata,
    pub filename: String,
    pub sheet_name: String,
    pub sheet_number: usize,
    pub sheet_count: usize,
    pub signex_version: &'static str,
}

#[derive(Debug, Error)]
pub enum SubstitutionError {
    #[error("malformed substitution token: {0}")]
    Malformed(String),
}

/// Resolve every `${...}` token in `input` against `ctx`. Unknown tokens
/// render as empty string; strings that look like tokens but aren't
/// (containing whitespace, etc.) pass through verbatim.
///
/// Placeholder body — real regex-based implementation lands with the PDF PR.
pub fn resolve(
    _input: &str,
    _ctx: &SubstitutionContext<'_>,
) -> Result<String, SubstitutionError> {
    todo!("substitution resolver — implemented in the PDF pipeline PR")
}
