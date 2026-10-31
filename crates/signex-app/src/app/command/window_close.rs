//! Close Document, resolved against the window the command came from.
//!
//! Strokes route globally and the handlers act on the main window's active
//! tab, so Ctrl+F4 typed in another window used to close a document the user
//! was not looking at (#1787). Close Document is per-window, like Undo / Redo.

use iced::Task;

use crate::app::Message;
use crate::app::Signex;
use crate::app::state::WindowKind;

impl Signex {
    /// Close Document typed in `window`, or `None` when `window` is the main
    /// window (or names none) and the ordinary handler applies.
    ///
    /// - Undocked tab: close that window's own document through
    ///   `close_tab_parking_dirty` (dirty drafts are parked), which also
    ///   closes the window.
    /// - Component Preview: close the window as the OS close button does
    ///   (`WindowCloseRequested`); the main window's tabs are untouched.
    /// - Detached modal or panel: hosts no document, so nothing happens.
    pub(super) fn close_document_in_window(
        &mut self,
        window: Option<iced::window::Id>,
    ) -> Option<Task<Message>> {
        let id = window?;
        if self.ui_state.main_window_id == Some(id) {
            return None;
        }
        match self.ui_state.windows.get(&id)? {
            WindowKind::UndockedTab { path, .. } => {
                let idx = self
                    .document_state
                    .tabs
                    .iter()
                    .position(|t| &t.path == path);
                Some(match idx {
                    Some(idx) => self.close_tab_parking_dirty(idx),
                    None => Task::none(),
                })
            }
            WindowKind::ComponentEditor { .. } => Some(self.dispatch_update(Message::Window(
                crate::app::WindowMsg::WindowCloseRequested(id),
            ))),
            WindowKind::DetachedModal(_) | WindowKind::DetachedPanel(_) => Some(Task::none()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::command::CommandArgs;
    use crate::app::{TabInfo, TabKind};
    use crate::keymap::AppCommandId;

    fn tab(name: &str) -> TabInfo {
        let path = std::path::PathBuf::from(format!("/tmp/{name}.snxsch"));
        TabInfo {
            title: name.to_string(),
            path,
            cached_document: None,
            project_id: None,
            kind: TabKind::Schematic,
        }
    }

    fn app_with_undocked_b() -> (Signex, iced::window::Id) {
        let (mut app, _boot) = Signex::new();
        app.document_state.tabs = vec![tab("a"), tab("b")];
        app.document_state.active_tab = 0;
        let win = iced::window::Id::unique();
        app.ui_state.windows.insert(
            win,
            WindowKind::UndockedTab {
                path: app.document_state.tabs[1].path.clone(),
                title: "b".to_string(),
            },
        );
        (app, win)
    }

    fn close_document() -> AppCommandId {
        AppCommandId::new("close_active_document").unwrap()
    }

    /// #1787: Ctrl+F4 in an undocked window closes that window's document
    /// and leaves the main window's active tab open.
    #[test]
    fn close_document_in_an_undocked_window_closes_its_own_tab() {
        let (mut app, win) = app_with_undocked_b();
        let _ = app.dispatch_command(&close_document(), CommandArgs::none(), Some(win));
        let titles: Vec<_> = app
            .document_state
            .tabs
            .iter()
            .map(|t| t.title.as_str())
            .collect();
        assert_eq!(
            titles,
            ["a"],
            "the undocked window's tab closed, main's stayed"
        );
        assert_eq!(app.document_state.active_tab, 0);
    }

    /// The main window still closes its own active tab.
    #[test]
    fn close_document_in_the_main_window_closes_the_active_tab() {
        let (mut app, _win) = app_with_undocked_b();
        let main = app.ui_state.main_window_id;
        let _ = app.dispatch_command(&close_document(), CommandArgs::none(), main);
        let titles: Vec<_> = app
            .document_state
            .tabs
            .iter()
            .map(|t| t.title.as_str())
            .collect();
        assert_eq!(titles, ["b"]);
    }

    /// A Component Preview or detached panel window never closes a main
    /// window tab.
    #[test]
    fn close_document_in_a_non_document_window_leaves_the_main_tabs() {
        let (mut app, _win) = app_with_undocked_b();
        let editor = iced::window::Id::unique();
        app.ui_state.windows.insert(
            editor,
            WindowKind::ComponentEditor {
                library_path: std::path::PathBuf::from("/tmp/parts.snxlib"),
                table: "Resistors".to_string(),
                row_id: signex_library::RowId::new(),
            },
        );
        let panel = iced::window::Id::unique();
        app.ui_state.windows.insert(
            panel,
            WindowKind::DetachedPanel(crate::panels::PanelKind::Projects),
        );
        for window in [editor, panel] {
            let _ = app.dispatch_command(&close_document(), CommandArgs::none(), Some(window));
            assert_eq!(app.document_state.tabs.len(), 2);
        }
    }
}
