//! The sheet and project benches every row of this batch shares.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_app::active_bar::{ActiveBarAction, ActiveBarMsg};
use signex_app::app::{EditMsg, LoadedProject, Message, ProjectId, Signex, TabInfo, TabKind};
use signex_app::canvas::{BoxDirection, CanvasEvent};
use signex_app::dock::DockMessage;
use signex_app::panels::PanelMsg;
use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::{
    ChildSheet, FillType, Label, LabelType, LibSymbol, Point, SchematicSheet, SelectedItem,
    SelectedKind, Symbol, Wire,
};
use uuid::Uuid;

use crate::settle_support::settle;

pub(super) fn empty_sheet() -> SchematicSheet {
    SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        ..Default::default()
    }
}

/// A part with an embedded library body, which ERC and the panels read.
pub(super) fn part(reference: &str, at: (f64, f64)) -> Symbol {
    Symbol {
        uuid: Uuid::now_v7(),
        lib_id: "lib:r".to_string(),
        reference: reference.to_string(),
        value: "10k".to_string(),
        position: Point::new(at.0, at.1),
        unit: 1,
        ..Default::default()
    }
}

pub(super) fn wire(a: (f64, f64), b: (f64, f64)) -> Wire {
    Wire {
        uuid: Uuid::now_v7(),
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        ..Default::default()
    }
}

pub(super) fn net_label(text: &str, at: (f64, f64)) -> Label {
    Label::new(text, Point::new(at.0, at.1), LabelType::Net)
}

fn with_body(sheet: &mut SchematicSheet) {
    let body: LibSymbol = serde_json::from_str(r#"{"id":"lib:r"}"#).expect("a library body");
    sheet.lib_symbols.insert(body.id.clone(), body);
}

/// An app with one open sheet at its save point.
pub(super) struct Bench {
    pub app: Signex,
    pub path: PathBuf,
    _dir: tempfile::TempDir,
}

pub(super) fn bench(mut sheet: SchematicSheet) -> Bench {
    with_body(&mut sheet);
    let _ = signex_app::diagnostics::init_logging();
    let dir = tempfile::TempDir::new().expect("tempdir");
    let path = dir.path().join("audit.snxsch");
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.snap_enabled = false;
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
    Bench {
        app,
        path,
        _dir: dir,
    }
}

impl Bench {
    pub fn post(&mut self, message: Message) {
        let _ = self.app.update(message);
    }

    pub fn panel(&mut self, msg: PanelMsg) {
        self.post(Message::Dock(DockMessage::Panel(msg)));
    }

    pub fn arm(&mut self, action: ActiveBarAction) {
        self.post(Message::ActiveBar(ActiveBarMsg::Action(action)));
    }

    pub fn click(&mut self, x: f64, y: f64) {
        self.post(Message::CanvasEvent(CanvasEvent::Clicked {
            world_x: x,
            world_y: y,
        }));
    }

    pub fn undo(&mut self) {
        self.post(Message::Edit(EditMsg::Undo));
    }

    pub fn redo(&mut self) {
        self.post(Message::Edit(EditMsg::Redo));
    }

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

    pub fn steps(&self) -> usize {
        self.app
            .document_state
            .active_engine()
            .expect("an open sheet")
            .undo_position()
    }

    pub fn select(&mut self, items: Vec<SelectedItem>) {
        self.app.interaction_state.active_canvas_mut().selected = items;
    }

    pub fn select_parts(&mut self, references: &[&str]) {
        let items = references
            .iter()
            .map(|r| {
                let symbol = self
                    .sheet()
                    .symbols
                    .iter()
                    .find(|s| s.reference == *r)
                    .expect("the part is on the sheet");
                SelectedItem::new(symbol.uuid, SelectedKind::Symbol)
            })
            .collect();
        self.select(items);
    }

    /// Select what a box over the whole sheet touches.
    pub fn select_everything(&mut self) {
        self.post(Message::CanvasEvent(CanvasEvent::BoxSelect {
            x1: -50.0,
            y1: -50.0,
            x2: 450.0,
            y2: 450.0,
            direction: BoxDirection::RightToLeft,
        }));
    }
}

/// A sheet symbol named `designator` placing `child.snxsch`.
pub(super) fn sheet_symbol(designator: &str) -> ChildSheet {
    ChildSheet {
        uuid: Uuid::now_v7(),
        name: designator.to_string(),
        filename: "child.snxsch".to_string(),
        position: Point::new(20.0, 20.0),
        size: (40.64, 43.18),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    }
}

/// `root.snxsch` and `child.snxsch`, both open as tabs of one project, each
/// at its save point; the root is the active tab.
pub(super) struct ProjectBench {
    pub app: Signex,
    pub root: PathBuf,
    pub child: PathBuf,
    pub file: PathBuf,
    pub project: ProjectId,
    _dir: tempfile::TempDir,
}

fn write_sheet(path: &Path, sheet: &SchematicSheet) {
    let text = signex_types::format::SnxSchematic::new(sheet.clone())
        .write_string()
        .expect("encode sheet");
    std::fs::write(path, text).expect("write sheet");
}

fn sheet_entry(filename: &str) -> SheetEntry {
    SheetEntry {
        name: filename.to_string(),
        filename: filename.to_string(),
        symbols_count: 0,
        wires_count: 0,
        labels_count: 0,
        unknown: Default::default(),
    }
}

pub(super) fn project_bench(mut root: SchematicSheet, mut child: SchematicSheet) -> ProjectBench {
    with_body(&mut root);
    with_body(&mut child);
    let _ = signex_app::diagnostics::init_logging();
    let dir = tempfile::TempDir::new().expect("tempdir");
    let root_path = dir.path().join("root.snxsch");
    let child_path = dir.path().join("child.snxsch");
    write_sheet(&root_path, &root);
    write_sheet(&child_path, &child);
    let mut data: ProjectData = serde_json::from_value(serde_json::json!({
        "name": "dcdc-buck",
        "dir": dir.path().to_string_lossy(),
        "schematic_root": "root.snxsch",
        "pcb_file": null
    }))
    .expect("a project");
    data.sheets = vec![sheet_entry("root.snxsch"), sheet_entry("child.snxsch")];
    let file = dir.path().join("dcdc-buck.snxprj");
    signex_types::project::write_project(&file, &data).expect("write project");

    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.snap_enabled = false;
    let project = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id: project,
        path: file.clone(),
        data,
        pending_libraries: HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(project);
    for (title, path, sheet) in [("root", &root_path, root), ("child", &child_path, child)] {
        let mut engine =
            signex_engine::Engine::new_with_path(sheet, Some(path.clone())).expect("engine");
        engine.mark_saved();
        app.document_state.engines.insert(path.clone(), engine);
        app.document_state.tabs.push(TabInfo {
            title: title.to_string(),
            path: path.clone(),
            cached_document: None,
            project_id: Some(project),
            kind: TabKind::Schematic,
        });
    }
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(root_path.clone());
    ProjectBench {
        app,
        root: root_path,
        child: child_path,
        file,
        project,
        _dir: dir,
    }
}

impl ProjectBench {
    /// Send `message` and run every task it starts.
    pub fn send(&mut self, message: Message) {
        settle(&mut self.app, message);
    }

    pub fn post(&mut self, message: Message) {
        let _ = self.app.update(message);
    }

    pub fn panel(&mut self, msg: PanelMsg) {
        self.post(Message::Dock(DockMessage::Panel(msg)));
    }

    pub fn doc(&self, path: &Path) -> &SchematicSheet {
        self.app.document_state.engines[path].document()
    }

    pub fn data(&self) -> &ProjectData {
        &self
            .app
            .document_state
            .project_by_id(self.project)
            .expect("the project")
            .data
    }

    pub fn data_mut(&mut self) -> &mut ProjectData {
        let id = self.project;
        &mut self
            .app
            .document_state
            .projects
            .iter_mut()
            .find(|p| p.id == id)
            .expect("the project")
            .data
    }

    pub fn is_dirty(&self, path: &Path) -> bool {
        self.app.document_state.is_document_dirty(path)
    }

    /// One step back on the project's General history.
    pub fn undo_project(&mut self) -> bool {
        let id = self.project;
        let document = &mut self.app.document_state;
        let project = document
            .projects
            .iter_mut()
            .find(|p| p.id == id)
            .expect("the project");
        let tab = document.project_tabs.get_mut(&id).expect("a history");
        tab.general_history.undo(&mut project.data)
    }

    pub fn project_can_undo(&self) -> bool {
        self.app
            .document_state
            .project_tabs
            .get(&self.project)
            .is_some_and(|tab| tab.general_history.can_undo())
    }
}
