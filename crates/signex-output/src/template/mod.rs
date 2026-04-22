//! Sheet templates — `.snxsht` format. See `OUTPUT_PLAN.md` §4.
//!
//! Ten portrait/landscape ISO (A0-A5) + five ANSI (A-E) built-in templates
//! ship with the binary, unpacked to the user's config dir on first run.

use std::collections::BTreeMap;

use thiserror::Error;

mod builtin;
mod format;

/// Opaque identifier for a template. Built-ins use well-known strings
/// (`"iso_a4_landscape"`, `"ansi_c_landscape"`, etc.); user templates use a
/// project-relative path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TemplateId(pub String);

/// In-memory representation of a parsed `.snxsht` template. Fields are
/// filled in by the format parser when it lands; this is the public surface
/// so other modules can reference the type.
#[derive(Debug, Clone, Default)]
pub struct Template {
    pub id: TemplateId,
    pub display_name: String,
    pub page_size: crate::pdf::PageSize,
    pub orientation: crate::pdf::Orientation,
    pub title_block_fields: BTreeMap<String, TitleBlockField>,
}

#[derive(Debug, Clone)]
pub struct TitleBlockField {
    pub x_mm: f64,
    pub y_mm: f64,
    pub font_family: String,
    pub font_size_mm: f64,
    pub default_text: String,
}

impl Default for TemplateId {
    fn default() -> Self {
        Self("iso_a4_landscape".to_string())
    }
}

impl Default for crate::pdf::PageSize {
    fn default() -> Self {
        crate::pdf::PageSize::IsoA4
    }
}

impl Default for crate::pdf::Orientation {
    fn default() -> Self {
        crate::pdf::Orientation::Landscape
    }
}

#[derive(Debug, Error)]
pub enum TemplateError {
    #[error("template not found: {0:?}")]
    NotFound(TemplateId),

    #[error("template parse error: {0}")]
    Parse(String),
}
