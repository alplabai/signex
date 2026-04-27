//! Sheet templates — `.snxsht` format. See `OUTPUT_PLAN.md` §4.
//!
//! A template describes how a page is framed: page size, orientation,
//! border margin, optional zone markers, title-block layout with
//! substitution-aware default text for each field. Templates are rendered
//! on top of the schematic content during PDF export and print preview.
//!
//! 17 built-in templates (ISO A0-A5 + ANSI A-E, portrait + landscape where
//! standard practice allows) ship with the binary — see `builtin.rs`. User
//! templates (`.snxsht` files on disk) are a later concern; `format.rs` is
//! reserved for the parser/emitter when custom templates ship.

use thiserror::Error;

use crate::pdf::{Orientation, PageSize};

pub mod builtin;
pub mod format;

pub use builtin::{all_builtin_ids, load_builtin};
pub use format::{SnxshtError, emit_template, parse_template};

/// Opaque identifier for a template. Built-ins use well-known strings
/// (`"iso_a4_landscape"`, `"ansi_c_landscape"`); user templates will later
/// use a project-relative path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TemplateId(pub String);

impl Default for TemplateId {
    fn default() -> Self {
        Self("iso_a4_landscape".to_string())
    }
}

impl From<&str> for TemplateId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct Template {
    pub id: TemplateId,
    pub display_name: String,
    pub page_size: PageSize,
    pub orientation: Orientation,
    pub frame: Frame,
    pub title_block: TitleBlock,
}

/// Page frame — the border drawn around the schematic area plus optional
/// zone markers (A/B/C… letters and 1/2/3… digits around the edge).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    /// Margin between the page edge and the schematic area, in mm.
    pub border_margin_mm: f64,
    pub show_zone_markers: bool,
    pub horizontal_zones: u8,
    pub vertical_zones: u8,
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            border_margin_mm: 10.0,
            show_zone_markers: true,
            horizontal_zones: 8,
            vertical_zones: 6,
        }
    }
}

/// Title block — a rectangle anchored at the bottom-right of the page
/// containing substitution-aware text fields.
#[derive(Debug, Clone)]
pub struct TitleBlock {
    pub width_mm: f64,
    pub height_mm: f64,
    pub fields: Vec<TitleBlockField>,
}

/// One text field inside a title block. `x_mm` / `y_mm` are relative to the
/// title block's top-left corner.
#[derive(Debug, Clone)]
pub struct TitleBlockField {
    pub name: String,
    pub x_mm: f64,
    pub y_mm: f64,
    pub font_family: String,
    pub font_size_mm: f64,
    pub font_style: FontStyle,
    /// Default text, may contain `${TOKEN}` substitutions.
    pub default_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontStyle {
    Normal,
    Bold,
    Italic,
    BoldItalic,
}

#[derive(Debug, Error)]
pub enum TemplateError {
    #[error("template not found: {0:?}")]
    NotFound(TemplateId),
}
