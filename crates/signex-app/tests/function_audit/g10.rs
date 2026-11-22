//! Function audit, wave A batch g10: mockup rows m75 to m88 (Annotate, Reset,
//! Re-prefix, Number Sheets, Sheet Sync, Assign Footprints, Update PCB, Back
//! Annotate, Re-Annotate Board, ERC, Messages, DRC). The shared project and
//! board set-ups are here; the rows are in `g10/`.

#[path = "g10/annotate.rs"]
mod annotate;
#[path = "g10/boards.rs"]
mod boards;
#[path = "g10/sheets.rs"]
mod sheets;
#[path = "g10/validation.rs"]
mod validation;

use std::path::{Path, PathBuf};

use signex_app::annotate_undo::AnnotateUndo;
use signex_app::app::{
    EditMsg, FileMsg, LoadedProject, Message, ProjectId, Signex, TabDocument, TabInfo, TabKind,
};
use signex_app::keymap::AppCommandId;
use signex_app::menu_bar::MenuMessage;
use signex_types::format::{SnxPcb, SnxSchematic};
use signex_types::pcb::{Footprint, PcbBoard, Point as BoardPoint, SymbolLink};
use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::{ChildSheet, FillType, InstancePath, Point, SchematicSheet, Symbol};
use tempfile::TempDir;
use uuid::Uuid;

use crate::settle_support::settle;

const BOARD_FILE: &str = "Proj.snxpcb";

/// A project on disk with its root sheet open and in front; every other
/// sheet closed. The board, when given, is named by the project and closed.
struct Rig {
    app: Signex,
    project: ProjectId,
    root: PathBuf,
    prj: PathBuf,
    dir: PathBuf,
    board: PathBuf,
    _tmp: TempDir,
}

fn part(uuid: Uuid, reference: &str, value: &str, x: f64) -> Symbol {
    Symbol {
        uuid,
        lib_id: "Device:R".to_string(),
        reference: reference.to_string(),
        value: value.to_string(),
        position: Point::new(x, 0.0),
        in_bom: true,
        on_board: true,
        ..Default::default()
    }
}

fn placement(uuid: Uuid, name: &str, filename: &str, x: f64) -> ChildSheet {
    ChildSheet {
        uuid,
        name: name.to_string(),
        filename: filename.to_string(),
        position: Point::new(x, 50.0),
        size: (10.0, 10.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    }
}

fn sheet(symbols: Vec<Symbol>, child_sheets: Vec<ChildSheet>) -> SchematicSheet {
    SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols,
        child_sheets,
        ..Default::default()
    }
}

fn write_sheet(path: &Path, content: &SchematicSheet) {
    let text = SnxSchematic::new(content.clone())
        .write_string()
        .expect("encode sheet");
    std::fs::write(path, text).expect("write sheet");
}

fn entry(filename: &str) -> SheetEntry {
    SheetEntry {
        name: filename.to_string(),
        filename: filename.to_string(),
        symbols_count: 0,
        wires_count: 0,
        labels_count: 0,
        unknown: Default::default(),
    }
}

fn project_data(dir: &Path, sheets: &[&str], pcb_file: Option<&str>) -> ProjectData {
    ProjectData {
        document_id: None,
        description: None,
        name: "Proj".to_string(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: sheets.first().map(|name| (*name).to_string()),
        pcb_file: pcb_file.map(str::to_string),
        sheets: sheets.iter().map(|name| entry(name)).collect(),
        variant_definitions: Vec::new(),
        active_variant: None,
        libraries: Vec::new(),
        enable_git: false,
        rules: Default::default(),
        net_classes: Default::default(),
        annotation: None,
        schematic_defaults: Default::default(),
        project_defaults: Default::default(),
        agent_journal: Default::default(),
        schematic_design: Default::default(),
        fab_outputs: None,
        router_tool_diameter: None,
        requirements: Default::default(),
        parameters: Default::default(),
        unknown: Default::default(),
    }
}

/// A fresh app that starts from no user ERC overrides.
fn fresh_app() -> Signex {
    let _ = signex_app::diagnostics::init_logging();
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    app.ui_state.annotate_undo = AnnotateUndo::Project;
    app.ui_state.erc_severity_override.clear();
    app.ui_state.pin_matrix_overrides.clear();
    app
}

fn rig(root: SchematicSheet, closed: &[(&str, SchematicSheet)], board: Option<PcbBoard>) -> Rig {
    rig_on(fresh_app(), root, closed, board)
}

/// [`rig`] on an app already set up (a mounted library).
fn rig_on(
    mut app: Signex,
    root: SchematicSheet,
    closed: &[(&str, SchematicSheet)],
    board: Option<PcbBoard>,
) -> Rig {
    let tmp = TempDir::new().expect("tempdir");
    let dir = tmp.path().to_path_buf();
    let root_path = dir.join("root.snxsch");
    write_sheet(&root_path, &root);
    let mut names = vec!["root.snxsch"];
    for (name, content) in closed {
        write_sheet(&dir.join(name), content);
        names.push(*name);
    }
    let board_path = dir.join(BOARD_FILE);
    let names_board = board.is_some();
    if let Some(board) = board {
        let text = SnxPcb::new(board).write_string().expect("encode board");
        std::fs::write(&board_path, text).expect("write board");
    }
    let data = project_data(&dir, &names, names_board.then_some(BOARD_FILE));
    let prj = dir.join("Proj.snxprj");
    signex_types::project::write_project(&prj, &data).expect("write project");

    let project = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id: project,
        path: prj.clone(),
        data,
        pending_libraries: Default::default(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(project);
    let engine =
        signex_engine::Engine::new_with_path(root, Some(root_path.clone())).expect("engine");
    app.document_state.engines.insert(root_path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "root".to_string(),
        path: root_path.clone(),
        cached_document: None,
        project_id: Some(project),
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(root_path.clone());
    Rig {
        app,
        project,
        root: root_path,
        prj,
        dir,
        board: board_path,
        _tmp: tmp,
    }
}

/// A board with no project, opened from disk.
struct BoardRig {
    app: Signex,
    path: PathBuf,
    _tmp: TempDir,
}

fn open_board(board: PcbBoard) -> BoardRig {
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("board.snxpcb");
    let text = SnxPcb::new(board).write_string().expect("encode board");
    std::fs::write(&path, text).expect("write board");
    let mut app = fresh_app();
    settle(&mut app, Message::File(FileMsg::Opened(Some(path.clone()))));
    BoardRig {
        app,
        path,
        _tmp: tmp,
    }
}

fn empty_board() -> PcbBoard {
    serde_json::from_value(serde_json::json!({ "uuid": Uuid::nil() })).expect("board")
}

/// A footprint at `(x, y)` linked to schematic part `unit` on the root sheet.
fn footprint(
    uuid: Uuid,
    reference: &str,
    value: &str,
    (x, y): (f64, f64),
    unit: Uuid,
) -> Footprint {
    let mut fp: Footprint = serde_json::from_value(serde_json::json!({
        "uuid": uuid, "reference": reference, "value": value,
        "position": {"x": 0.0, "y": 0.0}, "layer": "TopCopper"
    }))
    .expect("footprint");
    fp.position = BoardPoint::new(x, y);
    fp.schematic_link = Some(SymbolLink::new(InstancePath::root(), [unit]));
    fp
}

/// The board at `path` as its tab holds it.
fn board_of<'a>(app: &'a Signex, path: &Path) -> &'a PcbBoard {
    app.document_state
        .tabs
        .iter()
        .find(|tab| tab.path == path)
        .and_then(|tab| tab.cached_document.as_ref())
        .and_then(TabDocument::as_pcb)
        .expect("the board is open in a tab")
}

fn menu(app: &mut Signex, id: &str) {
    let id = AppCommandId::new(id).expect("a catalog command");
    settle(app, Message::Menu(MenuMessage::Command(id)));
}

fn undo(app: &mut Signex) {
    settle(app, Message::Edit(EditMsg::Undo));
}

/// Bring the tab of `path` to the front.
fn show(app: &mut Signex, path: &Path) {
    let index = app
        .document_state
        .tabs
        .iter()
        .position(|tab| tab.path == path)
        .expect("a tab for the path");
    let window_id = app.ui_state.main_window_id.expect("main window");
    settle(
        app,
        Message::Tab {
            window_id,
            msg: signex_app::tab_bar::TabMessage::Select(index),
        },
    );
}

/// The symbol `id` on the sheet an engine holds at `path`, if it is loaded.
fn loaded_symbol<'a>(app: &'a Signex, path: &Path, id: Uuid) -> Option<&'a Symbol> {
    app.document_state
        .engines
        .get(path)?
        .document()
        .symbols
        .iter()
        .find(|symbol| symbol.uuid == id)
}

fn reference(app: &Signex, path: &Path, id: Uuid) -> String {
    loaded_symbol(app, path, id)
        .expect("the sheet is loaded and holds the part")
        .reference
        .clone()
}
