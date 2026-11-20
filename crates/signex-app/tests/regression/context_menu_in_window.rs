//! #2267 — a canvas menu row picked in an undocked sheet window runs against
//! that window. Place ▸ Sheet Entry was refused while the main window showed
//! a board, because the row was gated by the main window; Sheet Actions ▸
//! Sync Sheet Entries read the main window's selection and sheet, and Create
//! Sheet Symbol From Sheet placed on the main window's sheet once its file
//! dialog answered.

use std::path::{Path, PathBuf};

use signex_app::app::{
    ContextAction, ContextMenuMsg, Message, Signex, TabInfo, TabKind, Tool, WindowMsg,
};
use signex_app::io_ports::{HeadlessAnswers, IoPorts};
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
    let symbol = Uuid::now_v7();
    let mut app = app_with_empty_sheet();
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
    app.document_state.engines.insert(
        PathBuf::from(SHEET),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    let (mut app, window) = undock_beside_a_board(app, Path::new(SHEET));
    app.interaction_state
        .canvases
        .get_mut(&window)
        .expect("the window's canvas")
        .selected = vec![SelectedItem::new(symbol, SelectedKind::ChildSheet)];

    let _ = pick_row(&mut app, window, "sync_sheet_entries");

    let dialog = app
        .ui_state
        .sheet_sync
        .as_ref()
        .expect("the sync dialog opened");
    assert_eq!(dialog.parent, PathBuf::from(SHEET), "the window's sheet");
    assert_eq!(dialog.symbol, symbol, "the window's selected sheet symbol");
    assert_eq!(
        app.document_state.active_path, None,
        "the main window still shows the board"
    );
}

fn write_sheet(path: &Path, sheet: SchematicSheet) {
    let text = signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("encode");
    std::fs::write(path, text).expect("write sheet");
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
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let engine =
        signex_engine::Engine::new_with_path(SchematicSheet::default(), Some(parent.clone()))
            .expect("engine");
    app.document_state.engines.insert(parent.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "parent".to_string(),
        path: parent.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(parent.clone());
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
