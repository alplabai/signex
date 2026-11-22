//! The benches and small drivers the wave C output rows share.

use std::path::{Path, PathBuf};

use pcb_geom::ClickMode;
use signex_app::app::{
    EditMsg, FileMsg, LoadedProject, Message, ProjectId, Signex, TabDocument, TabInfo, TabKind,
};
use signex_app::keymap::AppCommandId;
use signex_app::menu_bar::MenuMessage;
use signex_app::pcb_canvas::PcbCanvasMsg;
use signex_types::coord::Unit;
use signex_types::format::SnxPcb;
use signex_types::pcb::{PcbBoard, Point as BoardPoint};
use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::{LibSymbol, Point, SchematicSheet, Symbol};
use uuid::Uuid;

use crate::settle_support::settle;

pub(super) fn fresh_app() -> Signex {
    let _ = signex_app::diagnostics::init_logging();
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    app
}

/// Run catalog command `id` as a menu row does.
pub(super) fn command(app: &mut Signex, id: &str) {
    let id = AppCommandId::new(id).expect("a catalog command");
    settle(app, Message::Menu(MenuMessage::Command(id)));
}

pub(super) fn undo(app: &mut Signex) {
    settle(app, Message::Edit(EditMsg::Undo));
}

pub(super) fn redo(app: &mut Signex) {
    settle(app, Message::Edit(EditMsg::Redo));
}

/// Bring the tab of `path` to the front.
pub(super) fn show(app: &mut Signex, path: &Path) {
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

/// A part with a library body key, at `at`.
pub(super) fn part(reference: &str, value: &str, at: (f64, f64)) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "lib:r".into(),
        reference: reference.into(),
        value: value.into(),
        position: Point::new(at.0, at.1),
        unit: 1,
        in_bom: true,
        on_board: true,
        ..Default::default()
    }
}

/// An app with one sheet open at its save point.
pub(super) struct SheetBench {
    pub app: Signex,
    pub path: PathBuf,
    _dir: tempfile::TempDir,
}

pub(super) fn open_sheet(symbols: Vec<Symbol>) -> SheetBench {
    let mut sheet = SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols,
        ..Default::default()
    };
    let body: LibSymbol = serde_json::from_str(r#"{"id":"lib:r"}"#).expect("a library body");
    sheet.lib_symbols.insert(body.id.clone(), body);
    let dir = tempfile::TempDir::new().expect("tempdir");
    let path = dir.path().join("audit.snxsch");
    let mut app = fresh_app();
    let mut engine = signex_engine::Engine::new(sheet.clone()).expect("engine");
    engine.mark_saved();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "audit".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.clone());
    app.interaction_state
        .active_canvas_mut()
        .set_render_cache(Some(
            signex_app::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
        ));
    SheetBench {
        app,
        path,
        _dir: dir,
    }
}

impl SheetBench {
    pub fn sheet(&self) -> &SchematicSheet {
        self.app
            .document_state
            .active_engine()
            .expect("an open sheet")
            .document()
    }

    pub fn dirty(&self) -> bool {
        self.app.document_state.is_document_dirty(&self.path)
    }

    pub fn has_part(&self, reference: &str) -> bool {
        self.sheet()
            .symbols
            .iter()
            .any(|symbol| symbol.reference == reference)
    }

    pub fn part_id(&self, reference: &str) -> Uuid {
        self.sheet()
            .symbols
            .iter()
            .find(|symbol| symbol.reference == reference)
            .expect("the part is on the sheet")
            .uuid
    }
}

/// An app with `board` opened from its file, at its save point, snapping to
/// the grid only.
pub(super) struct BoardBench {
    pub app: Signex,
    pub path: PathBuf,
    pub main: iced::window::Id,
    _dir: tempfile::TempDir,
}

pub(super) fn open_board(board: PcbBoard) -> BoardBench {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let path = dir.path().join("audit.snxpcb");
    let text = SnxPcb::new(board).write_string().expect("encode board");
    std::fs::write(&path, text).expect("write board");
    let mut app = fresh_app();
    let main = app.ui_state.main_window_id.expect("main window");
    app.ui_state.unit = Unit::Mm;
    app.ui_state.snap_enabled = true;
    app.ui_state.pcb_snap.objects.clear();
    app.ui_state.pcb_snap.to_axes = false;
    settle(&mut app, Message::File(FileMsg::Opened(Some(path.clone()))));
    BoardBench {
        app,
        path,
        main,
        _dir: dir,
    }
}

impl BoardBench {
    pub fn board(&self) -> PcbBoard {
        self.app
            .document_state
            .tabs
            .iter()
            .find(|tab| tab.path == self.path)
            .and_then(|tab| tab.cached_document.as_ref())
            .and_then(TabDocument::as_pcb)
            .expect("the board is open")
            .clone()
    }

    pub fn json(&self) -> String {
        serde_json::to_string(&self.board()).expect("the board as JSON")
    }

    pub fn dirty(&self) -> bool {
        self.app.document_state.is_document_dirty(&self.path)
    }

    pub fn click(&mut self, x: f64, y: f64) {
        settle(
            &mut self.app,
            Message::PcbCanvas(PcbCanvasMsg::Clicked {
                at: BoardPoint::new(x, y),
                tolerance_mm: 0.05,
                mode: ClickMode::Replace,
            }),
        );
    }
}

/// A project's data naming `sheets` (the first is the root) and its board.
pub(super) fn project_data(dir: &Path, sheets: &[&str], pcb_file: Option<&str>) -> ProjectData {
    ProjectData {
        document_id: None,
        description: None,
        name: "Proj".to_string(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: sheets.first().map(|name| (*name).to_string()),
        pcb_file: pcb_file.map(str::to_string),
        sheets: sheets
            .iter()
            .map(|name| SheetEntry {
                name: (*name).to_string(),
                filename: (*name).to_string(),
                symbols_count: 0,
                wires_count: 0,
                labels_count: 0,
                unknown: Default::default(),
            })
            .collect(),
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

/// Load `data` as the app's active project, its file at `path`.
pub(super) fn add_project(app: &mut Signex, path: PathBuf, data: ProjectData) -> ProjectId {
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path,
        data,
        pending_libraries: Default::default(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(id);
    id
}
