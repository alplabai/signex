//! Text search and replace over one schematic sheet (#1268).
//!
//! Find/Replace used to read the canvas render cache and replace from the
//! text it had stored when the hit was found. Both are wrong for a search
//! that reaches sheets nobody has open: there is no render cache for them,
//! and the stored text is stale by the time Replace runs. This module works
//! on a [`SchematicSheet`] alone, so the same scanner serves the active
//! tab, a closed sheet read off-thread, and the live re-scan a replace does
//! just before it writes.
//!
//! A hit names *where* it is (`target`, `placement`), not *what it said*:
//! [`replace_commands`] re-reads the text from the document it is given.

mod fold;
mod replace;
mod scan;

#[cfg(test)]
mod tests;

pub use replace::{designator_commands, replace_all_ci, replace_commands};
pub use scan::scan_sheet;

use crate::TextTarget;
use signex_types::schematic::{InstancePath, Point, SelectedItem};

/// What kind of text a hit is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindKind {
    NetLabel,
    TextNote,
    Designator,
    Value,
}

impl FindKind {
    /// The name the results list shows.
    pub fn label(self) -> &'static str {
        match self {
            FindKind::NetLabel => "Net Label",
            FindKind::TextNote => "Text Note",
            FindKind::Designator => "Designator",
            FindKind::Value => "Value",
        }
    }
}

/// The placements of a sheet whose designators a scan reads.
///
/// A child sheet placed twice carries a designator per placement (#829), so
/// a scan of it yields one designator hit per placement.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DesignatorScope {
    /// Placements to read designators at. Empty reads the symbols'
    /// `reference` directly.
    pub placements: Vec<InstancePath>,
    /// The walk saw this sheet completely and found it placed at most once,
    /// so a designator edit may write `reference` instead of an instance
    /// entry (#1200).
    pub single: bool,
}

impl DesignatorScope {
    /// Read the designators of exactly one placement.
    pub fn at(placement: InstancePath, single: bool) -> Self {
        Self {
            placements: vec![placement],
            single,
        }
    }
}

/// One occurrence of the needle in a sheet.
#[derive(Debug, Clone, PartialEq)]
pub struct FindHit {
    pub kind: FindKind,
    pub item: SelectedItem,
    pub target: TextTarget,
    /// The text at scan time, for display only. A replace never reads it.
    pub text: String,
    /// The placement a designator hit was read at; `None` for every other
    /// kind and for a sheet scanned without placements.
    pub placement: Option<InstancePath>,
    /// See [`DesignatorScope::single`].
    pub single: bool,
    /// Where the item sits on the sheet, in millimetres.
    pub position: Point,
}
