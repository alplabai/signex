//! `.snxsht` user-template parser/emitter — placeholder.
//!
//! The legacy implementation was a Standard-style S-expression parser
//! built on top of `standard-parser::sexpr`. As part of the issue #62
//! Apache-clean cutover that codepath was removed; user-defined
//! templates will return when `.snxsht` is reimplemented on top of
//! Signex's native TOML-based format.
//!
//! The 17 built-in templates (`builtin.rs`) cover every shipping
//! template, so the loss is functionally the lack of *user-authored*
//! sheet templates. `parse_template` and `emit_template` stay as
//! exported symbols so any code paths referencing them keep
//! compiling; both currently surface `SnxshtError::NotImplemented`.
//!
//! TODO(issue#62): port the template format to TOML to match
//! `.snxsch`/`.snxpcb`/`.snxsym` once those wire formats stabilise.

use super::Template;

#[derive(Debug, thiserror::Error)]
pub enum SnxshtError {
    #[error(
        "user-defined .snxsht templates are not yet available in Signex Community; \
         use the built-in templates for now (issue #62)"
    )]
    NotImplemented,
}

/// Parse a `.snxsht` source string into a `Template`. Currently always
/// returns `SnxshtError::NotImplemented` — see module docs.
pub fn parse_template(_source: &str, _fallback_id: &str) -> Result<Template, SnxshtError> {
    Err(SnxshtError::NotImplemented)
}

/// Render a `Template` to its `.snxsht` string form. Currently a stub
/// that returns an empty string — the matching parser is a no-op so
/// round-trips are not meaningful until the format is reimplemented.
pub fn emit_template(_template: &Template) -> String {
    String::new()
}
