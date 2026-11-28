//! Step 2's drawn preview (#3171, mockup row 211): the parts of a new
//! import's staged library, with their symbol, footprint and alternates,
//! and the selected part's drawing, parameters, manufacturer and
//! suppliers. Pure; the reads run on workers in
//! `app/dispatch/library/import_dialog/parts.rs`.

use std::sync::Arc;

use signex_library::RowId;
use signex_library::primitive::PrimitiveRef;

use super::ImportDialogState;
use crate::panels::components_panel::details::ComponentDetails;

/// How many rows the part table draws; the caption says how many more.
pub const MAX_PART_ROWS: usize = 200;

/// The two views of step 2 for a staged preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PreviewTab {
    /// The part table and the selected part.
    #[default]
    Parts,
    /// The converter's report items.
    Report,
}

/// A distributor listing of a part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Supplier {
    pub distributor: String,
    pub sku: String,
}

/// One part of the staged library, as the part table lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewPart {
    pub table: String,
    pub row_id: RowId,
    pub part_number: String,
    /// The symbol's name; `None` when the part binds none or it is not in
    /// the library.
    pub symbol: Option<String>,
    pub footprint: Option<String>,
    /// The alternate footprints' names, in the part's order.
    pub alternates: Vec<String>,
    pub manufacturer: String,
    pub mpn: String,
    pub suppliers: Vec<Supplier>,
}

/// The part table's read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartsState {
    Loading,
    Ready(Vec<PreviewPart>),
    /// The staged library could not be read; why.
    Failed(String),
}

/// The selected part's symbol, footprints and parameters as the
/// Components panel reads them. Shared, compared by identity: the drawing
/// models are large and never edited.
#[derive(Debug, Clone)]
pub struct PartDrawing(pub Arc<ComponentDetails>);

impl PartialEq for PartDrawing {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// The selected part's read.
#[derive(Debug, Clone, PartialEq)]
pub enum PartRead {
    Loading,
    Ready(PartDrawing),
    Failed(String),
}

/// The selected part and its drawing.
#[derive(Debug, Clone, PartialEq)]
pub struct PartView {
    /// Its index in the part table.
    pub index: usize,
    pub read: PartRead,
    /// The footprint shown: the default, or an alternate picked.
    pub footprint: Option<PrimitiveRef>,
}

impl ImportDialogState {
    /// The staged library's parts, once read.
    pub fn preview_parts(&self) -> &[PreviewPart] {
        match &self.parts {
            Some(PartsState::Ready(parts)) => parts,
            _ => &[],
        }
    }

    /// The parts the table shows, with their index: those whose part
    /// number, symbol, footprint or manufacturer part number holds the
    /// filter text (case-insensitive).
    pub fn visible_parts(&self) -> Vec<(usize, &PreviewPart)> {
        let needle = self.part_filter.trim().to_lowercase();
        self.preview_parts()
            .iter()
            .enumerate()
            .filter(|(_, part)| {
                needle.is_empty()
                    || [
                        Some(&part.part_number),
                        part.symbol.as_ref(),
                        part.footprint.as_ref(),
                        Some(&part.mpn),
                    ]
                    .into_iter()
                    .flatten()
                    .any(|field| field.to_lowercase().contains(&needle))
            })
            .collect()
    }

    /// The selected part, while the filter shows it.
    pub fn selected_part(&self) -> Option<(&PartView, &PreviewPart)> {
        let view = self.part_view.as_ref()?;
        let part = self
            .visible_parts()
            .into_iter()
            .find(|(i, _)| *i == view.index)
            .map(|(_, part)| part)?;
        Some((view, part))
    }

    /// Step 2 has a drawn preview: a new import's staged library.
    pub fn has_drawn_preview(&self) -> bool {
        self.staged.is_some()
    }

    /// How many Alternate columns the table draws: the most alternates any
    /// part has, at most the mockup's two.
    pub fn alternate_columns(&self) -> usize {
        self.preview_parts()
            .iter()
            .map(|p| p.alternates.len())
            .max()
            .unwrap_or(0)
            .min(2)
    }

    /// Forget the drawn preview's reads, as when its folder goes.
    pub fn clear_drawn_preview(&mut self) {
        self.parts = None;
        self.part_view = None;
        self.part_filter.clear();
        self.preview_tab = PreviewTab::default();
    }
}
