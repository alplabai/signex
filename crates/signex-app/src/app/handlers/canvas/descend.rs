//! Opening a child sheet by descending through one of its sheet symbols, and
//! remembering which one (#1133): "Current page" and the print preview print
//! the designators of the placement the user came through, not a guess.

use std::path::PathBuf;

use iced::Task;
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use super::super::super::*;
use super::sheet_open::SheetOpen;

impl Signex {
    /// The canvas route (double-click, context menu, Enter on a selection):
    /// open the file `sheet_symbol` references and record that placement. A
    /// sheet already open is focused at once; otherwise the file is read on a
    /// worker and the tab is installed, and the placement recorded, when it
    /// arrives. A missing file offers to create it (#1275).
    pub(crate) fn descend_through_sheet_symbol(
        &mut self,
        sheet_symbol: Uuid,
        filename: &str,
    ) -> Task<Message> {
        let Some(path) = self.resolve_child_sheet_path(filename) else {
            return Task::none();
        };
        let placement = self
            .focused_sheet_placement()
            .map(|placement| placement.child(sheet_symbol));
        // An undocked window cannot switch sheets inside its swap. Its sheet
        // symbol and placement are resolved here, against the window's sheet;
        // the open itself is queued to run on the main window afterwards.
        if self.interaction_state.acting_window.is_some() {
            let path = self.resolve_held_spelling_now(path);
            // An open child counts even before it has a file on disk.
            let open = self.document_state.engines.contains_key(&path)
                || self.document_state.tabs.iter().any(|tab| tab.path == path);
            if !open && !path.exists() {
                crate::diagnostics::log_warning(format!(
                    "The sheet {} is missing; open its parent sheet in the main window to create it.",
                    path.display()
                ));
                return Task::none();
            }
            return Task::done(Message::Project(ProjectMsg::DescendRequested {
                symbol: sheet_symbol,
                path,
                placement,
            }));
        }
        self.descend_to_child_sheet(sheet_symbol, path, placement)
    }

    /// The rest of a descent once the child's path and placement are known:
    /// focus the sheet if it is open, else read it on a worker.
    pub(crate) fn descend_to_child_sheet(
        &mut self,
        sheet_symbol: Uuid,
        path: PathBuf,
        placement: Option<InstancePath>,
    ) -> Task<Message> {
        let mut open = SheetOpen::at(placement);
        open.create_for = Some(sheet_symbol);
        self.open_sheet(path, open)
    }

    pub(super) fn record_descent_placement(
        &mut self,
        path: PathBuf,
        placement: Option<InstancePath>,
    ) {
        let switched = self.document_state.tab_placements.get(&path) != placement.as_ref();
        self.document_state.tab_placements.record(&path, placement);
        // Jumping within the shown placement keeps an open edit's text.
        if switched {
            self.close_text_edit("the sheet is shown through another placement now");
        }
        // The canvas draws the designators of the placement just recorded.
        self.refresh_canvas_designators();
        // The net highlight is per placement: show it for this one.
        self.sync_net_highlight_to_canvases();
    }

    /// The focused sheet's own placement: the one recorded when it was
    /// descended into, else the root chain when the cached Navigator hierarchy
    /// starts a walk at it (the root, or a page nothing references). `None`
    /// for a referenced sheet opened directly, whose copy is unknown. Reads
    /// only state already in the Model: no file is touched.
    fn focused_sheet_placement(&self) -> Option<InstancePath> {
        // The sheet in view: inside a window's canvas event `active_path` is
        // the window's sheet while `active_tab` is still the main window's.
        let active = self
            .document_state
            .active_path
            .clone()
            .or_else(|| self.active_tab_path())?;
        if let Some(recorded) = self.document_state.tab_placements.get(&active) {
            return Some(recorded.clone());
        }
        let hierarchy = self.document_state.panel_ctx.navigator.hierarchy.as_ref()?;
        hierarchy.starts_a_walk(&active).then(InstancePath::root)
    }
}
