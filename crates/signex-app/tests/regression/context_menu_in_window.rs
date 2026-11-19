//! #2267 — a canvas menu row picked in an undocked sheet window runs against
//! that window. Place ▸ Sheet Entry was refused while the main window showed
//! a board, because the row was gated by the main window.

use std::path::PathBuf;

use signex_app::app::{
    ContextAction, ContextMenuMsg, Message, Signex, TabInfo, TabKind, Tool, WindowMsg,
};

use crate::library_place_symbol::app_with_empty_sheet;

/// The empty sheet undocked into its own window; the main window shows a
/// board.
fn sheet_window_beside_a_board() -> (Signex, iced::window::Id) {
    let mut app = app_with_empty_sheet();
    let sheet = PathBuf::from("place-364.snxsch");
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    let _ = app.update(Message::Window(WindowMsg::UndockTab(0)));
    let window = *app.ui_state.windows.keys().next().expect("undocked window");
    let _ = app.update(Message::Window(WindowMsg::UndockedTabOpened {
        path: sheet,
        id: window,
    }));
    app.document_state.tabs.push(TabInfo {
        title: "board".to_string(),
        path: PathBuf::from("board-2267.snxpcb"),
        cached_document: None,
        project_id: None,
        kind: TabKind::Pcb,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = None;
    (app, window)
}

fn in_window(window: iced::window::Id, inner: ContextMenuMsg) -> Message {
    Message::ContextMenu(ContextMenuMsg::ScopedToWindow(window, Box::new(inner)))
}

#[test]
fn place_sheet_entry_from_an_undocked_sheets_menu_arms_beside_a_board() {
    let (mut app, window) = sheet_window_beside_a_board();
    let _ = app.update(in_window(window, ContextMenuMsg::Show(8.0, 8.0)));
    let _ = app.update(in_window(
        window,
        ContextMenuMsg::Action(ContextAction::Command {
            id: signex_app::keymap::AppCommandId::new("place_sheet_entry").expect("catalog id"),
            project: None,
        }),
    ));
    assert_eq!(app.interaction_state.current_tool, Tool::SheetEntry);
}
