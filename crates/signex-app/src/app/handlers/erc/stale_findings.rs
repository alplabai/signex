//! Findings about objects an edit removed (#1768).

use signex_erc::LocationSpace;

use crate::app::Signex;

impl Signex {
    /// Drop the active sheet's findings whose primary or peer object is gone
    /// from the document, so neither a canvas marker nor a panel row outlives
    /// its object. It re-checks what the last run found; it never runs ERC.
    pub(crate) fn drop_findings_of_removed_objects(&mut self) {
        let Some(path) = self.document_state.active_path.clone() else {
            return;
        };
        let Some(engine) = self.document_state.active_engine() else {
            return;
        };
        let Some(findings) = self.ui_state.erc_violations_by_path.get_mut(&path) else {
            return;
        };
        let exists = |item: &Option<signex_types::schematic::SelectedItem>| {
            item.as_ref()
                .is_none_or(|item| engine.has_selected_items(std::slice::from_ref(item)))
        };
        let before = findings.len();
        // Only a finding in this sheet's own space names objects of this sheet.
        findings.retain(|v| {
            !matches!(
                v.location_space,
                LocationSpace::ThisSheet | LocationSpace::Net
            ) || (exists(&v.primary) && exists(&v.peer))
        });
        if findings.len() == before {
            return;
        }
        self.refresh_active_erc_from_cache(Some(&path));
        let rows = self.build_erc_diagnostic_entries().len();
        if self
            .ui_state
            .erc_focus_global_index
            .is_some_and(|i| i >= rows)
        {
            self.ui_state.erc_focus_global_index = rows.checked_sub(1);
        }
    }
}
