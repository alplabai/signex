//! The reference picker's state (mockup rows 195 and 196): typing `@` or
//! pressing Insert reference opens it over the editor. It lists the parts,
//! then the nets, of the design the project's last notebook read built,
//! filtered as you type, and `U3.` lists the pins of U3
//! (`signex_notebook::picker_entries`); Enter inserts the stored call.

use std::sync::Arc;

use signex_notebook::{DesignIndex, PICKER_LIMIT, PickerEntry, Reference, picker_entries};

use crate::app::Signex;
use crate::app::state::ProjectId;

/// The id of the picker's query field, focused when it opens.
pub(crate) const PICKER_INPUT_ID: &str = "notebook-reference-picker";

/// An open picker.
#[derive(Debug, Clone)]
pub struct Picker {
    pub query: String,
    /// The design the entries come from.
    pub design: Arc<DesignIndex>,
    /// The rows shown for `query`: at most [`PICKER_LIMIT`].
    pub entries: Vec<PickerEntry>,
    /// How many entries match, shown or not.
    pub matches: usize,
    /// The highlighted row.
    pub row: usize,
}

impl Picker {
    pub fn new(design: Arc<DesignIndex>) -> Self {
        let mut picker = Picker {
            query: String::new(),
            design,
            entries: Vec::new(),
            matches: 0,
            row: 0,
        };
        picker.set_query(String::new());
        picker
    }

    /// The query changed: list its entries again, the first highlighted.
    pub fn set_query(&mut self, query: String) {
        self.matches = picker_entries(&self.design, &query, usize::MAX).len();
        self.entries = picker_entries(&self.design, &query, PICKER_LIMIT);
        self.query = query;
        self.row = 0;
    }

    /// Move the highlight one row, staying on the list.
    pub fn step(&mut self, down: bool) {
        let last = self.entries.len().saturating_sub(1);
        self.row = if down {
            (self.row + 1).min(last)
        } else {
            self.row.saturating_sub(1)
        };
    }

    /// The reference of row `row`, or of the highlighted row.
    pub fn chosen(&self, row: Option<usize>) -> Option<Reference> {
        self.entries
            .get(row.unwrap_or(self.row))
            .map(|entry| entry.reference.clone())
    }
}

impl Signex {
    /// The design index of `id`'s last notebook read; empty while none is
    /// held for `id` (the read runs when the tab opens).
    pub(crate) fn notebook_design(&self, id: ProjectId) -> Arc<DesignIndex> {
        let annotations = &self.ui_state.annotations;
        annotations
            .target
            .as_ref()
            .filter(|(project, _)| *project == id)
            .and(annotations.scan.as_ref())
            .and_then(|scan| scan.design.clone())
            .unwrap_or_default()
    }
}
