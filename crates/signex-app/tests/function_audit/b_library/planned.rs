//! m60: Favorites, Snippets and To-Do are planned panels (2026-11-19
//! decision). They open from View ▸ Panels and change nothing. The Design
//! Notebook tab is recorded as not built and is not tested.

use signex_app::app::{Message, OverlayMsg};
use signex_app::panels::PanelKind;

use super::support::{fresh_app, open_sheet};

#[test]
fn each_planned_panel_opens_in_the_dock_and_leaves_the_sheet_clean() {
    for kind in [PanelKind::Favorites, PanelKind::Snippets, PanelKind::Todo] {
        let mut app = fresh_app();
        let (path, _tmp) = open_sheet(&mut app);

        let _ = app.update(Message::Overlay(OverlayMsg::OpenPanel(kind)));

        assert!(
            app.document_state.dock.locate(kind).is_some(),
            "{kind:?} is docked"
        );
        assert!(
            !app.document_state.is_document_dirty(&path),
            "{kind:?} dirtied the sheet"
        );
        let sheet = app.document_state.engines[&path].document();
        assert!(sheet.symbols.is_empty(), "{kind:?} changed the sheet");
    }
}
