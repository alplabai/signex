//! The Project tab Fonts section's own state (#1178, mockup row 205): the
//! licenses as typed, the last refusal and whether a file is being copied.
//! Plain state, UI-free (ADR-0006); never saved.

use std::collections::HashMap;

use signex_i18n_ids::Translatable;

/// The Fonts section's state, per project.
#[derive(Debug, Clone, Default)]
pub struct FontsTabState {
    /// Licenses as typed, by family (lower case), until Enter commits them
    /// or a section switch shows the stored values again.
    pub license_drafts: HashMap<String, String>,
    /// Why the last add or edit was refused; cleared by the next one.
    pub problem: Option<Translatable>,
    /// A file is being read and copied.
    pub adding: bool,
}

impl FontsTabState {
    /// Forget what was typed: the boxes show the stored values again.
    pub(crate) fn clear_drafts(&mut self) {
        self.license_drafts.clear();
        self.problem = None;
    }
}
