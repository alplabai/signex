//! #2267 — a canvas menu row picked in an undocked sheet window runs against
//! that window. Place ▸ Sheet Entry was refused while the main window showed
//! a board, because the row was gated by the main window; Sheet Actions ▸
//! Sync Sheet Entries read the main window's selection and sheet, and Create
//! Sheet Symbol From Sheet placed on the main window's sheet once its file
//! dialog answered.

use std::path::{Path, PathBuf};

use signex_app::app::{
    ContextAction, ContextMenuMsg, Message, SheetSyncMsg, Signex, TabInfo, TabKind, Tool, UiMsg,
    WindowMsg,
};
use signex_app::io_ports::{HeadlessAnswers, IoPorts};
use signex_app::keymap::AppCommandId;
use signex_app::menu_bar::MenuMessage;
use signex_app::test_adapter::boardless_tab;
use signex_types::schematic::{
    ChildSheet, FillType, Label, LabelType, Point, SchematicSheet, SelectedItem, SelectedKind,
};
use uuid::Uuid;

use crate::library_place_symbol::app_with_empty_sheet;
use crate::settle_support::settle_task;

const SHEET: &str = "place-364.snxsch";

/// The empty sheet undocked into its own window; the main window shows a
/// board.
fn sheet_window_beside_a_board() -> (Signex, iced::window::Id) {
    undock_beside_a_board(app_with_empty_sheet(), Path::new(SHEET))
}

/// `app`'s active sheet, at `sheet`, undocked into its own window; the main
/// window shows a board.
fn undock_beside_a_board(mut app: Signex, sheet: &Path) -> (Signex, iced::window::Id) {
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    let _ = app.update(Message::Window(WindowMsg::UndockTab(0)));
    let window = *app.ui_state.windows.keys().next().expect("undocked window");
    let _ = app.update(Message::Window(WindowMsg::UndockedTabOpened {
        path: sheet.to_path_buf(),
        id: window,
    }));
    app.document_state
        .tabs
        .push(boardless_tab("board", "board-2267.snxpcb", TabKind::Pcb));
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = None;
    (app, window)
}

fn in_window(window: iced::window::Id, inner: ContextMenuMsg) -> Message {
    Message::ContextMenu(ContextMenuMsg::ScopedToWindow(window, Box::new(inner)))
}

/// Pick the canvas menu row `id` in `window`'s menu; the row's task.
fn pick_row(app: &mut Signex, window: iced::window::Id, id: &str) -> iced::Task<Message> {
    let _ = app.update(in_window(window, ContextMenuMsg::Show(8.0, 8.0)));
    app.update(in_window(
        window,
        ContextMenuMsg::Action(ContextAction::Command {
            id: signex_app::keymap::AppCommandId::new(id).expect("catalog id"),
            project: None,
        }),
    ))
}

#[test]
fn place_sheet_entry_from_an_undocked_sheets_menu_arms_beside_a_board() {
    let (mut app, window) = sheet_window_beside_a_board();
    let _ = pick_row(&mut app, window, "place_sheet_entry");
    assert_eq!(app.interaction_state.current_tool, Tool::SheetEntry);
}

/// Sheet Actions ▸ Sync Sheet Entries in the undocked sheet opens the dialog
/// for the window's sheet and its selected sheet symbol, not for the board
/// the main window shows.
#[test]
fn sync_sheet_entries_from_an_undocked_sheets_menu_syncs_that_sheets_symbol() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (mut app, window, parent, symbol) = undocked_sheet_with_a_selected_symbol(dir.path());

    let _ = pick_row(&mut app, window, "sync_sheet_entries");

    let dialog = app
        .ui_state
        .sheet_sync
        .as_ref()
        .expect("the sync dialog opened");
    assert_eq!(dialog.parent, parent, "the window's sheet");
    assert_eq!(dialog.symbol, symbol, "the window's selected sheet symbol");
    assert_eq!(
        app.document_state.active_path, None,
        "the main window still shows the board"
    );
}

/// #2618 — Apply of a sync dialog opened in an undocked sheet window edits
/// that window's sheet, though the dialog answers on the main window, which
/// shows a board. It was refused as stale, because Apply wanted the main
/// window's active sheet to be the dialog's parent.
#[test]
fn sync_sheet_entries_apply_from_an_undocked_sheet_lands_on_that_sheet() {
    let dir = tempfile::tempdir().expect("temp dir");
    write_sheet(
        &dir.path().join("child.snxsch"),
        SchematicSheet {
            labels: vec![Label {
                shape: "input".to_string(),
                ..Label::new("SDA", Point::new(10.0, 10.0), LabelType::Hierarchical)
            }],
            ..SchematicSheet::default()
        },
    );
    let (mut app, window, parent, _symbol) = undocked_sheet_with_a_selected_symbol(dir.path());
    let entries = |app: &Signex| -> Vec<String> {
        app.document_state.engines[&parent].document().child_sheets[0]
            .pins
            .iter()
            .map(|pin| pin.name.clone())
            .collect()
    };

    let task = pick_row(&mut app, window, "sync_sheet_entries");
    settle_task(&mut app, task);
    let preview = app
        .ui_state
        .sheet_sync
        .as_ref()
        .and_then(|dialog| dialog.preview().cloned())
        .expect("the closed child was read and previewed");
    assert_eq!(preview.added, ["SDA"]);

    let task = app.update(Message::from(SheetSyncMsg::ApplyRequested));
    settle_task(&mut app, task);

    assert!(
        app.ui_state.sheet_sync.is_none(),
        "Apply ran and closed the dialog, not refused as stale"
    );
    assert_eq!(entries(&app), ["SDA"], "the window's sheet symbol synced");
    assert_eq!(
        app.document_state.active_path, None,
        "the main window still shows the board"
    );
    assert!(
        app.document_state.dirty_document_paths().contains(&parent),
        "the window's sheet is unsaved"
    );

    // Edit ▸ Undo in the window's own menu bar.
    let task = app.update(Message::Ui(UiMsg::MenuInWindow {
        window,
        msg: MenuMessage::Command(AppCommandId::new("undo").expect("catalog id")),
    }));
    settle_task(&mut app, task);
    assert!(entries(&app).is_empty(), "one undo step reverts the sync");
}

/// A sheet at `dir/parent.snxsch` placing `child.snxsch` through a sheet
/// symbol with no entries, undocked beside a board with the symbol selected
/// in its window. The sheet has a real folder: a child reference resolves
/// against the sheet's own directory, and a bare file name has none.
fn undocked_sheet_with_a_selected_symbol(dir: &Path) -> (Signex, iced::window::Id, PathBuf, Uuid) {
    let parent = dir.join("parent.snxsch");
    let symbol = Uuid::now_v7();
    let sheet = SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        child_sheets: vec![ChildSheet {
            uuid: symbol,
            name: "child".to_string(),
            filename: "child.snxsch".to_string(),
            position: Point::new(0.0, 0.0),
            size: (30.0, 30.0),
            stroke_width: 0.0,
            fill: FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins: Vec::new(),
            instances: Vec::new(),
        }],
        ..Default::default()
    };
    write_sheet(&parent, sheet.clone());
    let (mut app, window) = undock_beside_a_board(app_with_sheet_at(&parent, sheet), &parent);
    app.interaction_state
        .canvases
        .get_mut(&window)
        .expect("the window's canvas")
        .selected = vec![SelectedItem::new(symbol, SelectedKind::ChildSheet)];
    (app, window, parent, symbol)
}

fn write_sheet(path: &Path, sheet: SchematicSheet) {
    let text = signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("encode");
    std::fs::write(path, text).expect("write sheet");
}

/// `sheet` open at `path` in the one tab, which is active.
fn app_with_sheet_at(path: &Path, sheet: SchematicSheet) -> Signex {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let engine =
        signex_engine::Engine::new_with_path(sheet, Some(path.to_path_buf())).expect("engine");
    app.document_state
        .engines
        .insert(path.to_path_buf(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "parent".to_string(),
        path: path.to_path_buf(),
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.to_path_buf());
    app
}

/// Sheet Actions ▸ Create Sheet Symbol From Sheet in the undocked sheet: the
/// file dialog answers on the main window, which shows a board, and the
/// symbol still lands on the window's sheet.
#[test]
fn create_sheet_symbol_from_an_undocked_sheets_menu_lands_on_that_sheet() {
    let dir = tempfile::tempdir().expect("temp dir");
    let parent = dir.path().join("parent.snxsch");
    let power = dir.path().join("power.snxsch");
    write_sheet(&parent, SchematicSheet::default());
    write_sheet(
        &power,
        SchematicSheet {
            labels: vec![Label {
                shape: "input".to_string(),
                ..Label::new("VIN", Point::new(10.0, 10.0), LabelType::Hierarchical)
            }],
            ..SchematicSheet::default()
        },
    );
    let app = app_with_sheet_at(&parent, SchematicSheet::default());
    let (mut app, window) = undock_beside_a_board(app, &parent);
    let answers = HeadlessAnswers::default();
    app.ui_state.io = IoPorts::headless(answers.clone());
    answers.replace_all([("path".to_string(), power.display().to_string())]);

    let task = pick_row(&mut app, window, "create_sheet_symbol_from_sheet");
    settle_task(&mut app, task);

    let placed: Vec<String> = app.document_state.engines[&parent]
        .document()
        .child_sheets
        .iter()
        .map(|s| s.filename.clone())
        .collect();
    assert_eq!(
        placed,
        ["power.snxsch"],
        "the symbol is on the window's sheet"
    );
    assert_eq!(
        app.document_state.active_path, None,
        "the main window still shows the board"
    );
}
