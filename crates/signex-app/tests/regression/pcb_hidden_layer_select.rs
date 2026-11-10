//! #2271 review: a selection written from outside the board canvas (a DRC
//! row, Cross Select, Cross Probe) leaves out items on a hidden layer, so
//! Delete cannot remove what the user cannot see (#2223). The Properties
//! panel counts only the active tab's selection, and an undocked board
//! window's selection never lands in the main window's panel.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use pcb_geom::{ClickMode, ItemRef};
use signex_app::app::{FileMsg, LoadedProject, Message, PcbMsg, Signex, UiMsg, WindowMsg};
use signex_app::diagnostics;
use signex_app::keymap::{AppCommandId, KeyStroke, KeyToken, Modifiers};
use signex_app::menu_bar::MenuMessage;
use signex_app::pcb_canvas::PcbCanvasMsg;
use signex_app::tab_bar::TabMessage;
use signex_types::format::{SnxPcb, SnxSchematic};
use signex_types::pcb::{
    Footprint, LayerDef, Pad, PadShape, PadType, PcbBoard, Point, Segment, SymbolLink,
};
use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::{InstancePath, SchematicSheet, SelectedItem, SelectedKind, Symbol};
use uuid::Uuid;

use crate::drive::drive;
use crate::menu_support::menu_row;

const R5: Uuid = Uuid::from_u128(0x2271_0001);
const U_R5: Uuid = Uuid::from_u128(0x2271_0002);
const PAD: Uuid = Uuid::from_u128(0x2271_0003);

fn send(app: &mut Signex, message: Message) {
    let task = app.update(message);
    drive(app, task);
}

fn layer(id: u8, name: &str) -> LayerDef {
    LayerDef {
        id,
        name: name.to_owned(),
        layer_type: "signal".to_owned(),
        display_name: String::new(),
    }
}

fn track(n: u128, y: f64, layer: &str, net: u32) -> Segment {
    Segment {
        uuid: Uuid::from_u128(n),
        start: Point::new(0.0, y),
        end: Point::new(10.0, y),
        width: 0.25,
        layer: layer.to_owned(),
        net,
        unknown_columns: Default::default(),
    }
}

fn empty_board() -> PcbBoard {
    PcbBoard {
        sketch: None,
        uuid: Uuid::nil(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: vec![layer(0, "TopCopper"), layer(31, "BottomCopper")],
        setup: None,
        nets: Vec::new(),
        footprints: Vec::new(),
        segments: Vec::new(),
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        unknown: Default::default(),
    }
}

fn new_app() -> Signex {
    let _ = diagnostics::init_logging();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    app.ui_state
        .keymap_profiles
        .set_active_profile("altium")
        .expect("altium profile");
    app.ui_state.active_keymap = app.ui_state.keymap_profiles.compile_active().into();
    app
}

fn write_board(dir: &Path, name: &str, board: PcbBoard) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, SnxPcb::new(board).write_string().unwrap()).unwrap();
    path
}

fn open(app: &mut Signex, path: &Path) {
    send(
        app,
        Message::File(FileMsg::Opened(Some(path.to_path_buf()))),
    );
}

fn tab_index(app: &Signex, path: &Path) -> usize {
    app.document_state
        .tabs
        .iter()
        .position(|t| t.path == path)
        .expect("tab")
}

/// Make the tab of `path` the main window's active tab.
fn show(app: &mut Signex, path: &Path) {
    let index = tab_index(app, path);
    let window_id = app.ui_state.main_window_id.expect("main window");
    send(
        app,
        Message::Tab {
            window_id,
            msg: TabMessage::Select(index),
        },
    );
}

/// Undock the tab of `path` into its own window.
fn undock(app: &mut Signex, path: &Path) -> iced::window::Id {
    let index = tab_index(app, path);
    let before: HashSet<iced::window::Id> = app.ui_state.windows.keys().copied().collect();
    let _ = app.update(Message::Window(WindowMsg::UndockTab(index)));
    let window = *app
        .ui_state
        .windows
        .keys()
        .find(|id| !before.contains(id))
        .expect("undocked window");
    let _ = app.update(Message::Window(WindowMsg::UndockedTabOpened {
        path: path.to_path_buf(),
        id: window,
    }));
    window
}

/// Hide Bottom Layer on the board in the main window: make it the active
/// layer, then hide the active layer.
fn hide_bottom(app: &mut Signex) {
    send(app, menu_row("prev_layer"));
    send(app, menu_row("toggle_layer_visibility"));
}

fn main_selection(app: &Signex) -> Vec<ItemRef> {
    app.interaction_state
        .pcb_selection
        .selection()
        .items()
        .collect()
}

fn click(app: &mut Signex, window: Option<iced::window::Id>, y: f64, mode: ClickMode) {
    let msg = PcbCanvasMsg::Clicked {
        at: Point::new(5.0, y),
        tolerance_mm: 0.05,
        mode,
    };
    let message = match window {
        None => Message::PcbCanvas(msg),
        Some(window) => Message::Pcb(PcbMsg::CanvasInWindow { window, msg }),
    };
    send(app, message);
}

fn board_dump(app: &Signex) -> String {
    let docs = &app.document_state;
    format!("{:?}", docs.tabs[docs.active_tab].cached_document)
}

fn delete_key() -> Message {
    Message::Ui(UiMsg::KeymapStroke {
        window: None,
        stroke: KeyStroke {
            modifiers: Modifiers::default(),
            key: KeyToken::Delete,
        },
    })
}

/// Finding 4: with Bottom Layer hidden, a DRC row whose items are on it
/// selects nothing, says so, and Delete then leaves the board alone.
#[test]
fn a_drc_pick_on_a_hidden_layer_selects_nothing_and_delete_deletes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let mut board = empty_board();
    // A 0.15 mm gap on the bottom layer: one clearance violation.
    board.segments = vec![
        track(1, 0.0, "BottomCopper", 1),
        track(2, 0.4, "BottomCopper", 2),
    ];
    let path = write_board(tmp.path(), "board.snxpcb", board);
    let mut app = new_app();
    open(&mut app, &path);
    send(&mut app, Message::Pcb(PcbMsg::DrcRequested));
    let violations = app
        .interaction_state
        .pcb_drc
        .done_for(&path)
        .map(|d| d.report.violations.len());
    assert_eq!(violations, Some(1));

    // Shown: the row selects both tracks.
    send(&mut app, Message::Pcb(PcbMsg::DrcViolationPicked(0)));
    assert_eq!(main_selection(&app).len(), 2);

    hide_bottom(&mut app);
    assert!(
        main_selection(&app).is_empty(),
        "hiding drops the selection"
    );
    let since = diagnostics::newest_entry_id().unwrap_or(0);
    send(&mut app, Message::Pcb(PcbMsg::DrcViolationPicked(0)));
    assert!(
        main_selection(&app).is_empty(),
        "nothing hidden is selected"
    );
    let noticed = diagnostics::recent_entries()
        .iter()
        .any(|e| e.id > since && e.message.contains("hidden layer"));
    assert!(noticed, "the Messages panel says the items are hidden");
    assert_eq!(
        app.document_state
            .panel_ctx
            .pcb_board
            .as_ref()
            .map(|b| b.selected),
        Some(0)
    );

    let before = board_dump(&app);
    send(&mut app, delete_key());
    assert_eq!(board_dump(&app), before, "Delete removes nothing");
    assert_eq!(app.document_state.dirty_document_count(), 0);
}

/// R5 on the sheet, its footprint on the bottom of the board, in one project.
struct Linked {
    app: Signex,
    sheet: PathBuf,
    board: PathBuf,
    _tmp: tempfile::TempDir,
}

fn linked() -> Linked {
    let tmp = tempfile::tempdir().unwrap();
    let sheet = SchematicSheet {
        uuid: Uuid::from_u128(0x2271_0000),
        paper_size: "A4".to_string(),
        symbols: vec![Symbol {
            uuid: R5,
            lib_id: "Device:R".to_string(),
            reference: "R5".to_string(),
            position: signex_types::schematic::Point::new(20.0, 20.0),
            ..Default::default()
        }],
        ..Default::default()
    };
    let pad = Pad {
        uuid: PAD,
        number: "1".into(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: Point::new(0.0, 0.0),
        size: Point::new(1.0, 1.0),
        drill: None,
        layers: vec!["BottomCopper".into()],
        net: None,
        roundrect_ratio: 0.0,
        rotation: 0.0,
        net_locked: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    let mut board = empty_board();
    board.footprints = vec![Footprint {
        uuid: U_R5,
        reference: "R5".into(),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(10.0, 10.0),
        rotation: 0.0,
        layer: "BottomCopper".into(),
        locked: false,
        pads: vec![pad],
        graphics: Vec::new(),
        properties: Vec::new(),
        schematic_link: Some(SymbolLink::new(InstancePath::root(), [R5])),
        library_ref: None,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }];
    let sheet_path = tmp.path().join("demo.snxsch");
    let sheet_text = SnxSchematic::new(sheet).write_string().unwrap();
    std::fs::write(&sheet_path, sheet_text).unwrap();
    let board_path = write_board(tmp.path(), "demo.snxpcb", board);
    let mut app = new_app();
    open(&mut app, &sheet_path);
    open(&mut app, &board_path);
    add_project(&mut app, tmp.path());
    Linked {
        app,
        sheet: sheet_path,
        board: board_path,
        _tmp: tmp,
    }
}

fn add_project(app: &mut Signex, dir: &Path) {
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: dir.join("demo.snxprj"),
        data: ProjectData {
            document_id: None,
            description: None,
            name: "demo".to_string(),
            dir: dir.to_string_lossy().into_owned(),
            schematic_root: Some("demo.snxsch".to_string()),
            pcb_file: Some("demo.snxpcb".to_string()),
            sheets: vec![SheetEntry {
                name: "demo.snxsch".to_string(),
                filename: "demo.snxsch".to_string(),
                symbols_count: 0,
                wires_count: 0,
                labels_count: 0,
                unknown: Default::default(),
            }],
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            net_classes: Vec::new(),
            rules: None,
            annotation: None,
            schematic_defaults: Default::default(),
            project_defaults: Default::default(),
            parameters: Default::default(),
            unknown: Default::default(),
        },
        pending_libraries: Default::default(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    for tab in &mut app.document_state.tabs {
        tab.project_id = Some(id);
    }
}

fn select_all_on_the_sheet(app: &mut Signex) {
    let id = AppCommandId::new("select_all").expect("id");
    send(app, Message::Menu(MenuMessage::Command(id)));
    assert!(
        app.interaction_state
            .canvas
            .selected
            .contains(&SelectedItem::new(R5, SelectedKind::Symbol)),
        "Select All selects R5"
    );
}

fn cross_probe(app: &mut Signex) {
    let id = AppCommandId::new("cross_probe").expect("id");
    send(app, Message::Menu(MenuMessage::Command(id)));
}

/// S1: Cross Select from the sheet selects nothing of a bottom footprint
/// while the undocked board hides Bottom Layer.
#[test]
fn cross_select_from_the_sheet_skips_a_footprint_on_a_hidden_layer() {
    let mut f = linked();
    let board = f.board.clone();
    show(&mut f.app, &board);
    hide_bottom(&mut f.app);
    let window = undock(&mut f.app, &board);
    let sheet = f.sheet.clone();
    show(&mut f.app, &sheet);
    select_all_on_the_sheet(&mut f.app);
    let selected: Vec<ItemRef> = f
        .app
        .interaction_state
        .pcb_selections
        .get(&window)
        .map(|s| s.selection().items().collect())
        .unwrap_or_default();
    assert!(selected.is_empty(), "got {selected:?}");
}

/// S1: Tools ▸ Cross Probe selects the bottom footprint while Bottom Layer
/// is shown, and nothing once it is hidden.
#[test]
fn cross_probe_skips_a_footprint_on_a_hidden_layer() {
    let mut f = linked();
    let (sheet, board) = (f.sheet.clone(), f.board.clone());
    show(&mut f.app, &sheet);
    select_all_on_the_sheet(&mut f.app);
    cross_probe(&mut f.app);
    assert_eq!(main_selection(&f.app), vec![ItemRef::Footprint(U_R5)]);

    hide_bottom(&mut f.app);
    assert!(main_selection(&f.app).is_empty(), "hiding drops it");
    show(&mut f.app, &sheet);
    select_all_on_the_sheet(&mut f.app);
    cross_probe(&mut f.app);
    assert_eq!(
        f.app.document_state.tabs[f.app.document_state.active_tab].path,
        board
    );
    assert!(main_selection(&f.app).is_empty());
}

fn title(app: &Signex, path: &Path) -> String {
    app.document_state.tabs[tab_index(app, path)].title.clone()
}

/// Latest review: select on board A, switch to board B; B's Properties
/// shows B's board, not A's selection count.
#[test]
fn a_tab_switch_shows_the_other_boards_properties() {
    let tmp = tempfile::tempdir().unwrap();
    let mut a = empty_board();
    a.segments = vec![track(1, 0.0, "TopCopper", 1)];
    let mut b = empty_board();
    b.segments = vec![track(2, 0.0, "TopCopper", 1)];
    let a = write_board(tmp.path(), "a.snxpcb", a);
    let b = write_board(tmp.path(), "b.snxpcb", b);
    let mut app = new_app();
    open(&mut app, &a);
    open(&mut app, &b);
    show(&mut app, &a);
    click(&mut app, None, 0.0, ClickMode::Replace);
    let summary = app.document_state.panel_ctx.pcb_board.clone().expect("A");
    assert_eq!((summary.name, summary.selected), (title(&app, &a), 1));

    show(&mut app, &b);
    let summary = app.document_state.panel_ctx.pcb_board.clone().expect("B");
    assert_eq!((summary.name, summary.selected), (title(&app, &b), 0));
}

/// Latest review: a selection made in an undocked board window counts in
/// that window's Properties panel, not the main window's.
#[test]
fn an_undocked_boards_selection_stays_out_of_the_main_properties() {
    let tmp = tempfile::tempdir().unwrap();
    let mut a = empty_board();
    a.segments = vec![track(1, 0.0, "TopCopper", 1), track(2, 3.0, "TopCopper", 2)];
    let mut b = empty_board();
    b.segments = vec![track(3, 0.0, "TopCopper", 1)];
    let a = write_board(tmp.path(), "a.snxpcb", a);
    let b = write_board(tmp.path(), "b.snxpcb", b);
    let mut app = new_app();
    open(&mut app, &a);
    open(&mut app, &b);
    let window = undock(&mut app, &b);
    show(&mut app, &a);
    click(&mut app, None, 0.0, ClickMode::Replace);
    click(&mut app, None, 3.0, ClickMode::Add);
    assert_eq!(main_selection(&app).len(), 2);

    click(&mut app, Some(window), 0.0, ClickMode::Replace);
    let in_window = app.interaction_state.pcb_selections.get(&window);
    assert_eq!(in_window.map(|s| s.selection().len()), Some(1));

    let main = app
        .document_state
        .panel_ctx
        .pcb_board
        .clone()
        .expect("main");
    assert_eq!((main.name, main.selected), (title(&app, &a), 2));
    let undocked = app
        .document_state
        .window_panel_ctx
        .get(&window)
        .and_then(|ctx| ctx.pcb_board.clone())
        .expect("the window's board summary");
    assert_eq!((undocked.name, undocked.selected), (title(&app, &b), 1));
}
