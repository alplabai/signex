//! Opening a child sheet by descending through one of its sheet symbols, and
//! remembering which one (#1133): "Current page" and the print preview print
//! the designators of the placement the user came through, not a guess.

use std::path::PathBuf;

use signex_types::schematic::InstancePath;
use uuid::Uuid;

use super::super::super::*;

impl Signex {
    /// The canvas route (double-click, context menu, Enter on a selection):
    /// open the file `sheet_symbol` references and record that placement.
    pub(crate) fn descend_through_sheet_symbol(&mut self, sheet_symbol: Uuid, filename: &str) {
        let Some(path) = self.resolve_child_sheet_path(filename) else {
            return;
        };
        let placement = self
            .focused_sheet_placement()
            .map(|placement| placement.child(sheet_symbol));
        self.open_sheet_with_placement(path, placement);
    }

    /// Open or focus `path` and make `placement` the one its tab shows. `None`
    /// is a placement the caller could not determine: it clears a stale one
    /// rather than leaving the tab on a different copy than the user entered.
    /// Nothing changes when the sheet could not be opened.
    pub(crate) fn open_sheet_with_placement(
        &mut self,
        path: PathBuf,
        placement: Option<InstancePath>,
    ) {
        // Record under the spelling the tab is held at (#1149), not the
        // spelling the hierarchy resolved.
        let path = self.resolve_held_spelling_now(path);
        if self.open_or_focus_sheet_path(path.clone()) {
            self.document_state.tab_placements.record(&path, placement);
            // The canvas draws the designators of the placement just recorded.
            self.refresh_canvas_designators();
        }
    }

    /// The focused sheet's own placement: the one recorded when it was
    /// descended into, else the root chain when the cached Navigator hierarchy
    /// starts a walk at it (the root, or a page nothing references). `None`
    /// for a referenced sheet opened directly, whose copy is unknown. Reads
    /// only state already in the Model: no file is touched.
    fn focused_sheet_placement(&self) -> Option<InstancePath> {
        let active = self.active_tab_path()?;
        if let Some(recorded) = self.document_state.tab_placements.get(&active) {
            return Some(recorded.clone());
        }
        let hierarchy = self.settled_navigator_hierarchy()?;
        hierarchy.starts_a_walk(&active).then(InstancePath::root)
    }
}

#[cfg(test)]
mod tests {
    use crate::app::handlers::menu::export::tests::{app_workspace, sheet_with_net};
    use signex_types::format::SnxSchematic;

    /// While a Navigator rebuild is pending the hierarchy on screen may
    /// predate the edit, so a descent must not start its chain from it.
    #[test]
    fn a_pending_hierarchy_does_not_decide_the_focused_sheets_placement() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().join("root.snxsch");
        let text = SnxSchematic::new(sheet_with_net("R1", "N", &[]))
            .write_string()
            .expect("encode");
        std::fs::write(&root, text).expect("write");
        let mut app = app_workspace(&dir.path().to_string_lossy(), &["root.snxsch"]);
        app.open_or_focus_sheet_path(root);
        app.refresh_panel_ctx();
        app.settle_navigator();
        assert!(
            app.focused_sheet_placement().is_some(),
            "precondition: the settled hierarchy starts a walk at the root"
        );

        // A page is added: the hierarchy on screen is now the previous one.
        app.document_state.projects[0]
            .data
            .sheets
            .push(signex_types::project::SheetEntry {
                name: "T".into(),
                filename: "T.snxsch".into(),
                symbols_count: 0,
                wires_count: 0,
                labels_count: 0,
                unknown: Default::default(),
            });
        app.refresh_panel_ctx();

        assert!(!app.navigator_is_settled(), "precondition: rebuild pending");
        assert_eq!(app.focused_sheet_placement(), None);
    }
}
