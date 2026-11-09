//! #1312 — cross probe between the schematic and the board. Cross Select Mode
//! (on by default) follows a selection into the other side's windows, both
//! ways, without echoing it back; Cross Probe brings the other side forward
//! or opens it. Every entry point is driven through a UI message: the Tools
//! menu, both canvas menus, the palette and the Cross Select Mode key.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use pcb_geom::{ClickMode, ItemRef};
use signex_app::app::{
    CommandPaletteMsg, ContextAction, ContextMenuMsg, FileMsg, LoadedProject, Message, Signex,
    UiMsg, WindowMsg,
};
use signex_app::diagnostics;
use signex_app::keymap::{AppCommandId, KeyStroke, KeyToken, Modifiers};
use signex_app::menu_bar::MenuMessage;
use signex_app::pcb_canvas::PcbCanvasMsg;
use signex_app::tab_bar::TabMessage;
use signex_types::format::{SnxPcb, SnxSchematic};
use signex_types::pcb::{Footprint, Pad, PadShape, PadType, PcbBoard, Point, SymbolLink};
use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::{
    InstancePath, SchematicSheet, SelectedItem, SelectedKind, Symbol, Wire,
};
use uuid::Uuid;

const R1: Uuid = Uuid::from_u128(0x0131_2001);
const WIRE: Uuid = Uuid::from_u128(0x0131_2002);
const U_R1: Uuid = Uuid::from_u128(0x0131_2003);
const PAD: Uuid = Uuid::from_u128(0x0131_2004);

/// One resistor R1 and a wire clear of it.
fn sheet() -> SchematicSheet {
    SchematicSheet {
        uuid: Uuid::from_u128(0x0131_2000),
        paper_size: "A4".to_string(),
        symbols: vec![Symbol {
            uuid: R1,
            lib_id: "Device:R".to_string(),
            reference: "R1".to_string(),
            position: signex_types::schematic::Point::new(20.0, 20.0),
            ..Default::default()
        }],
        wires: vec![Wire {
            uuid: WIRE,
            start: signex_types::schematic::Point::new(50.0, 50.0),
            end: signex_types::schematic::Point::new(60.0, 50.0),
            stroke_width: 0.0,
            color: None,
            locked: false,
            unknown_columns: Default::default(),
        }],
        ..Default::default()
    }
}

/// R1's footprint at (10, 10), linked to R1 on the root sheet.
fn board() -> PcbBoard {
    let pad = Pad {
        uuid: PAD,
        number: "1".into(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: Point::new(0.0, 0.0),
        size: Point::new(1.0, 1.0),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: None,
        roundrect_ratio: 0.0,
        rotation: 0.0,
        net_locked: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    let footprint = Footprint {
        uuid: U_R1,
        reference: "R1".into(),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(10.0, 10.0),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        pads: vec![pad],
        graphics: Vec::new(),
        properties: Vec::new(),
        schematic_link: Some(SymbolLink::new(InstancePath::root(), [R1])),
        library_ref: None,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    PcbBoard {
        sketch: None,
        uuid: Uuid::nil(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: Vec::new(),
        setup: None,
        nets: Vec::new(),
        footprints: vec![footprint],
        segments: Vec::new(),
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        unknown: Default::default(),
    }
}

struct Fixture {
    app: Signex,
    sheet: PathBuf,
    board: PathBuf,
    main: iced::window::Id,
    _tmp: tempfile::TempDir,
}

fn send(app: &mut Signex, message: Message) {
    let task = app.update(message);
    crate::drive::drive(app, task);
}

fn command(id: &str) -> AppCommandId {
    AppCommandId::new(id).expect("a catalog id")
}

/// The project owning both files, its board named by `pcb_file`.
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

/// The sheet open in the main window; the board too when `open_board`
/// (behind the sheet).
fn fixture(open_board: bool) -> Fixture {
    let _ = diagnostics::init_logging();
    let tmp = tempfile::tempdir().unwrap();
    let sheet_path = tmp.path().join("demo.snxsch");
    let board_path = tmp.path().join("demo.snxpcb");
    let sheet_text = SnxSchematic::new(sheet()).write_string().unwrap();
    std::fs::write(&sheet_path, sheet_text).unwrap();
    std::fs::write(&board_path, SnxPcb::new(board()).write_string().unwrap()).unwrap();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    app.ui_state
        .keymap_profiles
        .set_active_profile("altium")
        .expect("altium profile");
    app.ui_state.active_keymap = app.ui_state.keymap_profiles.compile_active().into();
    send(
        &mut app,
        Message::File(FileMsg::Opened(Some(sheet_path.clone()))),
    );
    if open_board {
        send(
            &mut app,
            Message::File(FileMsg::Opened(Some(board_path.clone()))),
        );
    }
    add_project(&mut app, tmp.path());
    let mut f = Fixture {
        app,
        sheet: sheet_path,
        board: board_path,
        main,
        _tmp: tmp,
    };
    show_sheet(&mut f);
    f
}

fn show_sheet(f: &mut Fixture) {
    let path = f.sheet.clone();
    show_in_main(f, &path);
}

fn show_board(f: &mut Fixture) {
    let path = f.board.clone();
    show_in_main(f, &path);
}

/// Make the tab of `path` the main window's active tab.
fn show_in_main(f: &mut Fixture, path: &Path) {
    let index = f
        .app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == path)
        .expect("tab");
    send(
        &mut f.app,
        Message::Tab {
            window_id: f.main,
            msg: TabMessage::Select(index),
        },
    );
}

/// Undock the tab of `path` into its own window.
fn undock(f: &mut Fixture, path: &Path) -> iced::window::Id {
    let index = f
        .app
        .document_state
        .tabs
        .iter()
        .position(|t| t.path == path)
        .expect("tab");
    let before: HashSet<iced::window::Id> = f.app.ui_state.windows.keys().copied().collect();
    let _ = f.app.update(Message::Window(WindowMsg::UndockTab(index)));
    let window = *f
        .app
        .ui_state
        .windows
        .keys()
        .find(|id| !before.contains(id))
        .expect("undocked window");
    let _ = f.app.update(Message::Window(WindowMsg::UndockedTabOpened {
        path: path.to_path_buf(),
        id: window,
    }));
    window
}

fn select_all_on_the_sheet(f: &mut Fixture) {
    send(
        &mut f.app,
        Message::Menu(MenuMessage::Command(command("select_all"))),
    );
    let selected = &f.app.interaction_state.canvas.selected;
    assert!(
        selected.contains(&SelectedItem::new(R1, SelectedKind::Symbol)),
        "Select All selects R1, got {selected:?}"
    );
}

fn main_board_selection(f: &Fixture) -> Vec<ItemRef> {
    f.app
        .interaction_state
        .pcb_selection
        .selection()
        .items()
        .collect()
}

fn window_board_selection(f: &Fixture, window: iced::window::Id) -> Vec<ItemRef> {
    f.app
        .interaction_state
        .pcb_selections
        .get(&window)
        .map(|s| s.selection().items().collect())
        .unwrap_or_default()
}

fn click_footprint(f: &mut Fixture) {
    send(
        &mut f.app,
        Message::PcbCanvas(PcbCanvasMsg::Clicked {
            at: Point::new(10.0, 10.0),
            tolerance_mm: 0.05,
            mode: ClickMode::Replace,
        }),
    );
    assert!(
        !main_board_selection(f).is_empty(),
        "the click selects R1's footprint"
    );
}

fn active_path(f: &Fixture) -> Option<PathBuf> {
    let docs = &f.app.document_state;
    docs.tabs.get(docs.active_tab).map(|t| t.path.clone())
}

#[test]
fn cross_select_is_on_by_default() {
    let f = fixture(false);
    assert!(f.app.interaction_state.cross_select.enabled);
}

/// Selecting R1 on the sheet selects its footprint on the board shown in an
/// undocked window, without taking focus or switching the main window.
#[test]
fn a_part_selected_on_the_sheet_is_selected_on_the_board() {
    let mut f = fixture(true);
    let board = f.board.clone();
    let window = undock(&mut f, &board);
    show_sheet(&mut f);
    select_all_on_the_sheet(&mut f);
    assert_eq!(
        window_board_selection(&f, window),
        vec![ItemRef::Footprint(U_R1)]
    );
    assert_eq!(active_path(&f), Some(f.sheet.clone()), "no tab switch");
}

/// Selecting R1's footprint on the board selects R1 on the sheet shown in an
/// undocked window.
#[test]
fn a_footprint_selected_on_the_board_is_selected_on_the_sheet() {
    let mut f = fixture(true);
    let sheet = f.sheet.clone();
    let window = undock(&mut f, &sheet);
    show_board(&mut f);
    click_footprint(&mut f);
    assert_eq!(
        f.app.interaction_state.canvases[&window].selected,
        vec![SelectedItem::new(R1, SelectedKind::Symbol)]
    );
}

/// The board selection cross select wrote is not read back as a change: the
/// sheet keeps R1 and the wire (an echo would leave R1 alone), on this
/// update and the next.
#[test]
fn cross_select_does_not_echo_back() {
    let mut f = fixture(true);
    let board = f.board.clone();
    let window = undock(&mut f, &board);
    show_sheet(&mut f);
    select_all_on_the_sheet(&mut f);
    let sheet_selection = f.app.interaction_state.canvas.selected.clone();
    assert!(sheet_selection.contains(&SelectedItem::new(WIRE, SelectedKind::Wire)));
    send(&mut f.app, Message::noop());
    send(&mut f.app, Message::noop());
    assert_eq!(f.app.interaction_state.canvas.selected, sheet_selection);
    assert_eq!(
        window_board_selection(&f, window),
        vec![ItemRef::Footprint(U_R1)]
    );
}

/// With Cross Select Mode off (its Ctrl+Shift+X key) a sheet selection leaves
/// the board alone; Tools ▸ Cross Select Mode turns it back on.
#[test]
fn with_cross_select_off_nothing_follows() {
    let mut f = fixture(true);
    let board = f.board.clone();
    let window = undock(&mut f, &board);
    show_sheet(&mut f);
    let ctrl_shift_x = Message::Ui(UiMsg::KeymapStroke {
        window: None,
        stroke: KeyStroke {
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Modifiers::default()
            },
            key: KeyToken::Character("x".into()),
        },
    });
    send(&mut f.app, ctrl_shift_x);
    assert!(
        !f.app.interaction_state.cross_select.enabled,
        "the key turns it off"
    );
    select_all_on_the_sheet(&mut f);
    assert!(window_board_selection(&f, window).is_empty());

    send(
        &mut f.app,
        Message::Menu(MenuMessage::Command(command("toggle_cross_select_mode"))),
    );
    assert!(
        f.app.interaction_state.cross_select.enabled,
        "the menu row turns it on"
    );
}

/// Tools ▸ Cross Probe with the board closed opens it, brings it forward and
/// selects R1's footprint there.
#[test]
fn cross_probe_opens_a_closed_board() {
    let mut f = fixture(false);
    select_all_on_the_sheet(&mut f);
    send(
        &mut f.app,
        Message::Menu(MenuMessage::Command(command("cross_probe"))),
    );
    assert_eq!(
        active_path(&f),
        Some(f.board.clone()),
        "the board is open and in front"
    );
    assert_eq!(main_board_selection(&f), vec![ItemRef::Footprint(U_R1)]);
    assert!(f.app.interaction_state.cross_select.pending.is_none());
    assert!(f.app.document_state.error_notice.is_none());
}

/// The sheet canvas menu's Cross Probe row brings a board tab that sits
/// behind the sheet forward and selects there; passive cross select had
/// left the hidden tab alone.
#[test]
fn the_sheet_menu_row_brings_a_hidden_board_forward() {
    let mut f = fixture(true);
    select_all_on_the_sheet(&mut f);
    assert_eq!(
        active_path(&f),
        Some(f.sheet.clone()),
        "cross select opens nothing"
    );
    send(
        &mut f.app,
        Message::ContextMenu(ContextMenuMsg::Action(ContextAction::Command {
            id: command("cross_probe"),
            project: None,
        })),
    );
    assert_eq!(active_path(&f), Some(f.board.clone()));
    assert_eq!(main_board_selection(&f), vec![ItemRef::Footprint(U_R1)]);
}

/// The board canvas menu's Cross Probe row brings the sheet forward and
/// selects R1 there.
#[test]
fn the_board_menu_row_brings_the_sheet_forward() {
    let mut f = fixture(true);
    show_board(&mut f);
    click_footprint(&mut f);
    send(
        &mut f.app,
        Message::ContextMenu(ContextMenuMsg::Action(ContextAction::Command {
            id: command("cross_probe"),
            project: None,
        })),
    );
    assert_eq!(active_path(&f), Some(f.sheet.clone()));
    assert_eq!(
        f.app.interaction_state.canvas.selected,
        vec![SelectedItem::new(R1, SelectedKind::Symbol)]
    );
}

/// Cross Probe runs from the command palette too.
#[test]
fn the_palette_runs_cross_probe() {
    let mut f = fixture(true);
    select_all_on_the_sheet(&mut f);
    send(&mut f.app, Message::CommandPalette(CommandPaletteMsg::Open));
    send(
        &mut f.app,
        Message::CommandPalette(CommandPaletteMsg::QueryChanged("Cross Probe".into())),
    );
    send(
        &mut f.app,
        Message::CommandPalette(CommandPaletteMsg::ExecuteSelected),
    );
    assert_eq!(active_path(&f), Some(f.board.clone()));
    assert_eq!(main_board_selection(&f), vec![ItemRef::Footprint(U_R1)]);
}
