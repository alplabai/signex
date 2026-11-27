//! Opening a child sheet by descending through one of its sheet symbols, and
//! remembering which one (#1133): "Current page" and the print preview print
//! the designators of the placement the user came through, not a guess.

use std::path::PathBuf;

use iced::Task;
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use super::super::super::*;
use super::sheet_open::SheetOpen;

/// What a descent does when the sheet symbol's file is not on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OnMissingFile {
    /// Offer Create Sheet for it (#1275).
    OfferCreate,
    /// Log why and open nothing (#2847).
    Refuse,
}

/// Say why a sheet symbol's file cannot be opened, in the words Properties
/// and the context menu use.
pub(super) fn log_missing_sheet_refused(path: &std::path::Path) {
    let reason = signex_i18n::I18n::en_us()
        .message_plain(signex_i18n::ids::PROPERTIES_SHEET_OPEN_MISSING, &[]);
    crate::diagnostics::log_warning(format!("{reason} ({})", path.display()));
}

impl Signex {
    /// The file a sheet symbol on the sheet at `parent` resolves to. Lexical
    /// only: it reads no disk, and a refused reference has none.
    pub(crate) fn child_sheet_file(
        parent: &std::path::Path,
        filename: &str,
    ) -> Option<std::path::PathBuf> {
        crate::app::project_sheets::resolve_child_reference_quiet(parent.parent()?, filename).ok()
    }

    /// Open Sheet (double-click, context menu, Enter on a selection, the
    /// harness panel's Go to): open the file `sheet_symbol` references and
    /// record that placement. A missing file is refused with its reason;
    /// Create Sheet is its own action.
    pub(crate) fn open_sheet_through_symbol(
        &mut self,
        sheet_symbol: Uuid,
        filename: &str,
    ) -> Task<Message> {
        self.descend(sheet_symbol, filename, OnMissingFile::Refuse)
    }

    /// The Create Sheet route: open the file `sheet_symbol` references, or
    /// offer to create it when it is missing (#1275).
    pub(crate) fn descend_through_sheet_symbol(
        &mut self,
        sheet_symbol: Uuid,
        filename: &str,
    ) -> Task<Message> {
        self.descend(sheet_symbol, filename, OnMissingFile::OfferCreate)
    }

    /// Whether the file of the first selected sheet symbol in `window` is one
    /// the project tree marks as missing. Reads only the probed facts, never
    /// the disk.
    pub(crate) fn selected_sheet_file_missing(&self, window: iced::window::Id) -> bool {
        let canvas = self.interaction_state.canvas_for_window(window);
        let Some(item) = canvas
            .selected
            .iter()
            .find(|item| item.kind == signex_types::schematic::SelectedKind::ChildSheet)
        else {
            return false;
        };
        let Some(parent) = self
            .document_state
            .sheet_path_for_window(window, &self.ui_state)
        else {
            return false;
        };
        let Some(engine) = self.document_state.engines.get(parent) else {
            return false;
        };
        let projects = &self.panel_ctx_for(window).projects;
        (engine.document().child_sheets.iter())
            .find(|sheet| sheet.uuid == item.uuid)
            .and_then(|sheet| Self::child_sheet_file(parent, &sheet.filename))
            .is_some_and(|file| crate::panels::sheet_file_missing(projects, &file))
    }

    /// Show the file `sheet_symbol` references and record that placement. A
    /// sheet already open is focused at once; otherwise the file is read on a
    /// worker and the tab is installed, and the placement recorded, when it
    /// arrives. `on_missing` says what a file not on disk does.
    fn descend(
        &mut self,
        sheet_symbol: Uuid,
        filename: &str,
        on_missing: OnMissingFile,
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
                match on_missing {
                    OnMissingFile::Refuse => log_missing_sheet_refused(&path),
                    OnMissingFile::OfferCreate => crate::diagnostics::log_warning(format!(
                        "The sheet {} is missing; open its parent sheet in the main window to create it.",
                        path.display()
                    )),
                }
                return Task::none();
            }
            let (Some(window), Some(from)) = (
                self.interaction_state.acting_window,
                self.document_state.active_path.clone(),
            ) else {
                debug_assert!(false, "an acting window always has an active path");
                crate::diagnostics::log_warning(
                    "The descent was not done: the window has no sheet shown.",
                );
                return Task::none();
            };
            return Task::done(Message::Project(ProjectMsg::DescendRequested {
                window,
                from,
                symbol: sheet_symbol,
                path,
                placement,
            }));
        }
        match on_missing {
            OnMissingFile::OfferCreate => {
                self.descend_to_child_sheet(sheet_symbol, path, placement)
            }
            OnMissingFile::Refuse => {
                let mut open = SheetOpen::at(placement);
                open.refuse_missing = true;
                self.open_sheet(path, open)
            }
        }
    }

    /// A descent an undocked window queued, landing on the main window. The
    /// window or its sheet can have changed since the click (closed, shows
    /// another sheet, the symbol edited away or re-pointed, the sheet entered
    /// through another placement): then nothing is opened, and the Messages
    /// panel says why.
    pub(crate) fn descend_requested(
        &mut self,
        window: iced::window::Id,
        from: &std::path::Path,
        symbol: Uuid,
        path: PathBuf,
        placement: Option<InstancePath>,
    ) -> Task<Message> {
        if let Some(why) = self.descent_no_longer_applies(window, from, symbol, &path, &placement) {
            crate::diagnostics::log_warning(refused_descent_line(why, &path));
            return Task::none();
        }
        self.descend_to_child_sheet(symbol, path, placement)
    }

    /// Why a queued descent no longer applies, `None` while it still does.
    fn descent_no_longer_applies(
        &self,
        window: iced::window::Id,
        from: &std::path::Path,
        symbol: Uuid,
        path: &std::path::Path,
        placement: &Option<InstancePath>,
    ) -> Option<&'static str> {
        use crate::app::state::WindowKind;
        if !self.is_document_window(window) {
            return Some("the window it started in has closed");
        }
        if !matches!(
            self.ui_state.windows.get(&window),
            Some(WindowKind::UndockedTab { path, .. }) if path == from
        ) {
            return Some("the window shows another sheet now");
        }
        let child = self
            .document_state
            .engines
            .get(from)?
            .document()
            .child_sheets
            .iter()
            .find(|child| child.uuid == symbol);
        let Some(child) = child else {
            return Some("the sheet symbol is gone from the sheet");
        };
        let resolved = from
            .parent()
            .and_then(|dir| {
                crate::app::project_sheets::resolve_child_reference(dir, &child.filename)
            })
            .map(|resolved| self.resolve_held_spelling(resolved));
        if resolved.as_deref() != Some(path) {
            return Some("the sheet symbol references another file now");
        }
        let shown = self.shown_placement_of(from, self.panel_ctx_for(window));
        if shown.map(|p| p.child(symbol)) != *placement {
            return Some("the sheet is entered through another placement now");
        }
        None
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
        let switched = self.document_view.tab_placements.get(&path) != placement.as_ref();
        self.document_view.tab_placements.record(&path, placement);
        // Jumping within the shown placement keeps an open edit's text.
        if switched {
            self.close_text_edit("the sheet is shown through another placement now");
        }
        // The canvas draws the designators of the placement just recorded.
        self.refresh_canvas_designators();
        self.refresh_canvas_not_fitted();
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
        self.shown_placement_of(&active, &self.document_view.panel_ctx)
    }

    /// The placement the sheet at `active` is shown through (see
    /// [`Self::focused_sheet_placement`]).
    /// The Navigator hierarchy is read from `ctx`, the panel context of the
    /// window the sheet is shown in.
    fn shown_placement_of(
        &self,
        active: &std::path::Path,
        ctx: &crate::panels::PanelContext,
    ) -> Option<InstancePath> {
        if let Some(recorded) = self.document_view.tab_placements.get(active) {
            return Some(recorded.clone());
        }
        let hierarchy = ctx.navigator.hierarchy.as_ref()?;
        hierarchy.starts_a_walk(active).then(InstancePath::root)
    }
}

/// The Messages line for a queued descent that no longer applies. The reason
/// comes before the path because the Messages panel cuts a line at 160 bytes.
pub(crate) fn refused_descent_line(why: &str, path: &std::path::Path) -> String {
    format!(
        "The descent was not done: {why}. Repeat it in the window that shows the sheet. Sheet: {}",
        path.display()
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_refused_descent_line_keeps_its_reason_after_compaction() {
        let why = "the sheet is entered through another placement now";
        let path = std::path::PathBuf::from(format!("/{}/child.snxsch", "d".repeat(200)));
        let line = super::refused_descent_line(why, &path);
        let out = crate::diagnostics::compact_message(&line);
        assert_ne!(out, line, "the line must need compacting");
        assert!(out.contains(why), "{out}");
    }
}
