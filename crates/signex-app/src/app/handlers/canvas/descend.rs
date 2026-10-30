//! Opening a child sheet by descending through one of its sheet symbols, and
//! remembering which one (#1133): "Current page" and the print preview print
//! the designators of the placement the user came through, not a guess.

use std::path::PathBuf;

use iced::Task;
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use super::super::super::*;

/// A canvas descent whose child sheet is being read on a worker (#859): which
/// sheet symbol it came through and the placement its tab will show.
#[derive(Debug)]
pub struct PendingDescent {
    symbol: Uuid,
    placement: Option<InstancePath>,
    /// The read this descent waits for; any other completion for the path
    /// (a stale one) must leave it alone.
    generation: Option<u64>,
}

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
        // Record under the spelling the tab is held at (#1149), not the
        // spelling the hierarchy resolved.
        let path = self.resolve_held_spelling_now(path);
        if self.focus_open_sheet(&path) {
            self.record_descent_placement(path, placement);
            return Task::none();
        }
        // A rename or delete still moving the file would leave the new tab on
        // a path that is about to vanish.
        if let Err(refusal) = self.refuse_start_while_path_op(&path, "opened") {
            let detail = refusal.to_string();
            crate::diagnostics::log_warning(detail.clone());
            self.document_state.error_notice = Some(crate::app::state::ErrorNotice::open(detail));
            return Task::none();
        }
        let title = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Schematic".to_string());
        let task = self.begin_document_read(
            path.clone(),
            crate::app::document_read::ReadJob::ChildSchematic { title },
        );
        // The read this descent rides on: the one just started, or the one
        // already in flight for the file.
        let generation = self.document_state.pending_opens.generation_of(&path);
        self.ui_state.pending_descents.insert(
            path,
            PendingDescent {
                symbol: sheet_symbol,
                placement,
                generation,
            },
        );
        task
    }

    /// The read of `path` ended with the file missing: offer to create the
    /// sheet the descent came through.
    pub(crate) fn offer_to_create_missing_child(
        &mut self,
        path: &std::path::Path,
        generation: u64,
    ) {
        let Some(descent) = self.take_descent(path, generation) else {
            return;
        };
        self.open_create_child_sheet_dialog(descent.symbol, true);
    }

    /// The pending descent for `path`, only when it waits for read `generation`.
    fn take_descent(&mut self, path: &std::path::Path, generation: u64) -> Option<PendingDescent> {
        let waits = self
            .ui_state
            .pending_descents
            .get(path)
            .is_some_and(|d| d.generation == Some(generation));
        if waits {
            self.ui_state.pending_descents.remove(path)
        } else {
            None
        }
    }

    /// The read of `path` ended: when its tab is up, record the placement the
    /// descent came through. Anything else (a failed or stale read) leaves
    /// nothing to record.
    pub(crate) fn finish_descent(&mut self, path: &std::path::Path, generation: u64) {
        let Some(descent) = self.take_descent(path, generation) else {
            return;
        };
        if self.has_tab_for(path) {
            self.record_descent_placement(path.to_path_buf(), descent.placement);
        }
    }

    fn record_descent_placement(&mut self, path: PathBuf, placement: Option<InstancePath>) {
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
            self.record_descent_placement(path, placement);
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
        let hierarchy = self.document_state.panel_ctx.navigator.hierarchy.as_ref()?;
        hierarchy.starts_a_walk(&active).then(InstancePath::root)
    }
}
