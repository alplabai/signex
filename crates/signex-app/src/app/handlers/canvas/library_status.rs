//! The status bar segment of a Library Browser or component editor tab
//! (#2262): the library and what is selected in it, or the part the editor
//! shows and its lifecycle state.

use signex_i18n::{ids, t};
use signex_library::RowId;

use super::super::super::*;

impl Signex {
    /// The library segment for `window_id`, `None` on any other tab.
    pub(crate) fn library_status(&self, window_id: iced::window::Id) -> Option<String> {
        let tab = self
            .document_state
            .tab_shown_in_window(window_id, &self.ui_state)?;
        let i18n = &self.ui_state.i18n;
        if let Some(path) = tab.kind.as_library_browser() {
            let lib = self.library.library_at(path)?;
            let library = format!("{}.snxlib", lib.display_name);
            let selected = self
                .library
                .library_browsers
                .get(path)
                .and_then(|browser| browser.selected_row);
            let row = selected.and_then(|id| {
                lib.tables
                    .values()
                    .flatten()
                    .find(|row| RowId::from_uuid(row.row_id) == id)
            });
            let text = match row {
                Some(row) => t!(
                    i18n,
                    ids::LIBRARY_STATUS_SELECTED,
                    "library" => library,
                    "pn" => row.internal_pn.as_str()
                ),
                None => t!(
                    i18n,
                    ids::LIBRARY_STATUS_SUMMARY,
                    "library" => library,
                    "components" => lib.tables.values().map(Vec::len).sum::<usize>()
                ),
            };
            return Some(text.into_string());
        }
        let editor = tab.kind.as_component_editor()?;
        let row = self
            .library
            .library_at(&editor.library_path)?
            .tables
            .get(&editor.table)?
            .iter()
            .find(|row| RowId::from_uuid(row.row_id) == editor.row_id)?;
        let lifecycle = crate::library::editor::kit::lifecycle_label(row.state);
        Some(
            t!(
                i18n,
                ids::LIBRARY_STATUS_COMPONENT,
                "pn" => row.internal_pn.as_str(),
                "lifecycle" => lifecycle
            )
            .into_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::LibraryMessage;
    use crate::library::state::{LibraryBrowserState, OpenLibrary};

    const TABLE: &str = "R";

    /// An app whose main window shows a Library Browser tab on a one-row
    /// library, nothing selected.
    fn app_with_browser() -> (Signex, iced::window::Id, std::path::PathBuf, RowId) {
        let (mut app, _task) = Signex::new();
        let window = iced::window::Id::unique();
        app.ui_state.main_window_id = Some(window);
        let root = std::path::PathBuf::from("status-lib.snxlib");
        let row = crate::library::component_preview::test_row();
        let row_id = RowId::from_uuid(row.row_id);
        app.library.open_libraries.clear();
        app.library.open_libraries.push(OpenLibrary {
            root: root.clone(),
            display_name: "Status".into(),
            library_id: uuid::Uuid::now_v7(),
            tables: std::collections::HashMap::from([(TABLE.to_string(), vec![row])]),
            cached_components: Vec::new(),
            cached_symbols: Vec::new(),
            cached_footprints: Vec::new(),
            cached_sims: Vec::new(),
            cached_classes: Vec::new(),
            observed_stamp: None,
            dangling: Default::default(),
            own_edit_stamp: None,
            display: Default::default(),
            pricing: Default::default(),
        });
        let mut browser = LibraryBrowserState::new(root.clone());
        browser.active_table = Some(TABLE.to_string());
        app.library.library_browsers.insert(root.clone(), browser);
        app.document_state.tabs.push(crate::app::TabInfo {
            title: "Status".into(),
            path: root.clone(),
            cached_document: None,
            project_id: None,
            kind: crate::app::TabKind::LibraryBrowser(root.clone()),
        });
        app.document_state.active_tab = app.document_state.tabs.len() - 1;
        (app, window, root, row_id)
    }

    fn shown(app: &Signex, window: iced::window::Id) -> Option<String> {
        app.status_activity(window)
            .map(|a| a.text.replace(['\u{2068}', '\u{2069}'], ""))
    }

    /// #2262: a Library Browser tab's status bar names the library and its
    /// rows, and after a row click the selected Internal PN.
    #[test]
    fn a_library_browser_tab_names_the_library_and_the_selected_row() {
        let (mut app, window, root, row_id) = app_with_browser();
        assert_eq!(
            shown(&app, window).as_deref(),
            Some("Status.snxlib · 1 component")
        );
        let _ = app.update(Message::Library(LibraryMessage::BrowserSelectRow {
            library_path: root,
            table: TABLE.into(),
            row_id,
        }));
        assert_eq!(
            shown(&app, window).as_deref(),
            Some("Status.snxlib · 1 selected · U1")
        );
    }
}
