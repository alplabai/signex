//! Sheet templates — `.snxsht` format. See `OUTPUT_PLAN.md` §4.
//!
//! A template describes how a page is framed: page size, orientation,
//! border margin, optional zone markers, title-block layout with
//! substitution-aware default text for each field. `frame.rs` lowers a
//! template to drawables, shared by the canvas, PDF export and print preview.
//!
//! 17 built-in templates (ISO A0-A5 + ANSI A-E, portrait + landscape where
//! standard practice allows) ship with the binary — see `builtin.rs`. User
//! templates (`.snxsht` files on disk) are a later concern; `format.rs` is
//! reserved for the parser/emitter when custom templates ship.

use thiserror::Error;

use crate::pdf::{Orientation, PageSize};
use signex_types::schematic::SheetFrameSettings;

pub mod builtin;
pub mod format;
pub mod frame;

pub use builtin::{all_builtin_ids, load_builtin, standard_title_block};
pub use format::{SnxshtError, emit_template, parse_template};
pub use frame::{CELL_CAPTION_MM, FramePrimitive, FrameTextAnchor, lower_sheet_frame};

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
    /// Border and zone markers; off leaves only the title block.
    pub show_border: bool,
    pub show_zone_markers: bool,
    pub horizontal_zones: u8,
    pub vertical_zones: u8,
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            border_margin_mm: 10.0,
            show_border: true,
            show_zone_markers: true,
            horizontal_zones: 8,
            vertical_zones: 6,
        }
    }
}

impl Frame {
    /// This frame as a sheet's own settings ask for it (#1134): border on or
    /// off and the zone counts, clamped to `MAX_ZONES`. The margin stays the
    /// frame's; with both counts at 0 no zone strip is drawn.
    ///
    /// Precedence: the sheet decides the border and the zone counts, always,
    /// because Page Options edits them per sheet and every sheet carries a
    /// value (8 × 6 by default). A template contributes the margin and the
    /// title-block layout. A template that wants other default zone counts
    /// should seed them into the sheet when it is applied, not override the
    /// sheet at draw time.
    pub fn with_sheet_settings(self, settings: &SheetFrameSettings) -> Self {
        let settings = settings.clamped();
        Self {
            show_border: settings.show_border,
            show_zone_markers: self.show_zone_markers
                && (settings.horizontal_zones > 0 || settings.vertical_zones > 0),
            horizontal_zones: settings.horizontal_zones,
            vertical_zones: settings.vertical_zones,
            ..self
        }
    }
}

/// Title block — a rectangle anchored at the bottom-right of the page
/// containing substitution-aware text: a table of labelled [`cells`]
/// (#1201), and free-positioned [`fields`] for a layout without a grid.
///
/// [`cells`]: Self::cells
/// [`fields`]: Self::fields
#[derive(Debug, Clone)]
pub struct TitleBlock {
    pub width_mm: f64,
    pub height_mm: f64,
    pub fields: Vec<TitleBlockField>,
    pub cells: Vec<TitleBlockCell>,
}

/// One bordered cell of a title-block table: a small caption in its
/// top-left corner ("Title", "Rev", …) and a value under it. Coordinates
/// are relative to the title block's top-left corner. The border and the
/// caption are drawn even when the value resolves to nothing, so the table
/// keeps its shape and a blank cell can be filled in by hand.
#[derive(Debug, Clone)]
pub struct TitleBlockCell {
    pub x_mm: f64,
    pub y_mm: f64,
    pub w_mm: f64,
    pub h_mm: f64,
    pub caption: String,
    /// Value text, may contain `${TOKEN}` substitutions.
    pub value: String,
    pub font_family: String,
    /// The value's height; shrunk to fit the cell's width when it is long.
    pub value_size_mm: f64,
    pub value_style: FontStyle,
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
