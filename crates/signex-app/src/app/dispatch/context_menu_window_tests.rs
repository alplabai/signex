//! #2267: a command row of a canvas menu opened in an undocked window is
//! gated and run against that window, not the main one.

use std::path::{Path, PathBuf};

use crate::app::handlers::menu::export::tests as fx;
use crate::app::state::WindowKind;
use crate::app::*;

/// Sheet A undocked in its own window; the main window shows a board and
/// no sheet.
fn sheet_window_beside_a_board() -> (Signex, iced::window::Id) {
    let a = PathBuf::from("/w/cm/a.snxsch");
    let mut app = fx::app_workspace("/w/cm", &["a.snxsch"]);
    let window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    app.ui_state.windows.insert(
        window,
        WindowKind::UndockedTab {
            path: a.clone(),
            title: "a".to_string(),
        },
    );
    let sheet = fx::sheet_with_net("R1", "A_NET", &[]);
    let mut slot = crate::canvas::CanvasSlot::new();
    slot.set_render_cache(Some(
        crate::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
    ));
    fx::open_with(&mut app.document_state, &a, sheet);
    app.interaction_state.canvases.insert(window, slot);
    app.document_state.tabs.push(tab(&a, TabKind::Schematic));
    app.document_state
        .tabs
        .push(tab(Path::new("/w/cm/board.snxpcb"), TabKind::Pcb));
    app.document_state.active_tab = 1;
    app.document_state.active_path = None;
    (app, window)
}

fn tab(path: &Path, kind: TabKind) -> TabInfo {
    crate::test_adapter::boardless_tab("t", path, kind)
}

fn scoped(window: iced::window::Id, inner: ContextMenuMsg) -> Message {
    Message::ContextMenu(ContextMenuMsg::ScopedToWindow(window, Box::new(inner)))
}

#[test]
fn place_sheet_entry_from_an_undocked_sheets_menu_is_not_refused_by_the_main_board() {
    let (mut app, window) = sheet_window_beside_a_board();
    let _ = app.update(scoped(window, ContextMenuMsg::Show(5.0, 5.0)));
    assert_eq!(
        app.interaction_state
            .context_menu
            .as_ref()
            .and_then(|menu| menu.window),
        Some(window),
        "the menu records its window"
    );
    let _ = app.update(scoped(
        window,
        ContextMenuMsg::Action(ContextAction::Command {
            id: crate::keymap::AppCommandId::new("place_sheet_entry").expect("catalog id"),
            project: None,
        }),
    ));
    assert_eq!(
        app.interaction_state.current_tool,
        Tool::SheetEntry,
        "the sheet's tool armed"
    );
    assert!(app.interaction_state.context_menu.is_none());
}

/// The same row picked in the main window, which shows the board, stays
/// refused: the gate follows the window, it was not dropped.
#[test]
fn place_sheet_entry_from_the_main_boards_menu_is_still_refused() {
    let (mut app, _window) = sheet_window_beside_a_board();
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::Show(5.0, 5.0)));
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::Action(
        ContextAction::Command {
            id: crate::keymap::AppCommandId::new("place_sheet_entry").expect("catalog id"),
            project: None,
        },
    )));
    assert_ne!(app.interaction_state.current_tool, Tool::SheetEntry);
}
