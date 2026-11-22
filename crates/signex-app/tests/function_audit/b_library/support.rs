//! The library, symbol, sheet and project set-ups the b_library rows share.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use signex_app::app::{
    LoadedProject, Message, ProjectId, Signex, SymbolDocument, TabInfo, TabKind,
};
use signex_app::library::messages::{
    BrowserEditMsg, LibraryMessage, PrimitiveEdit, SymbolEditorMsg,
};
use signex_app::library::state::{EditRowModalState, LibraryBrowserState};
use signex_library::adapter::LibraryAdapter;
use signex_library::adapters::local_git::{LibraryInitOptions, LocalGitAdapter};
use signex_library::library_file::{FORMAT_TOKEN, LibrarySection, SnxlibManifest};
use signex_library::manifest::{LibraryMode, UsersConfig, WorkflowConfig};
use signex_library::{
    ComponentClass, ComponentRow, DatasheetRef, InternalPn, LifecycleState, ManufacturerPart,
    ParamMap, ParamValue, PlmReserved, PrimitiveRef, RowId, Symbol, SymbolFile, SymbolPin,
};
use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::SchematicSheet;
use tempfile::TempDir;
use uuid::Uuid;

use crate::settle_support::settle;

/// The table every rig row lives in.
pub(super) const TABLE: &str = "Resistors";
/// A measurement every rig row carries: 4.7 kΩ.
pub(super) const RATING: &str = "Rating";

/// A fresh app with no boot mounts and no remembered stock-copy target.
pub(super) fn fresh_app() -> Signex {
    let _ = signex_app::diagnostics::init_logging();
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    app.library.pending_mounts.clear();
    app.library.stock_copy_target = None;
    app
}

fn row(library_id: Uuid, n: usize) -> ComponentRow {
    let now = chrono::Utc::now();
    let mut parameters = ParamMap::new();
    parameters.insert(
        RATING.to_string(),
        ParamValue::Measurement {
            value: 4700.0,
            unit: "ohm".to_string(),
        },
    );
    ComponentRow {
        alternate_symbols: Vec::new(),
        row_id: Uuid::now_v7(),
        server_revision: None,
        internal_pn: InternalPn::new(part_number(n)),
        class: ComponentClass::generic(),
        datasheet: DatasheetRef::default(),
        datasheet_label: String::new(),
        description: String::new(),
        more_datasheets: Vec::new(),
        state: LifecycleState::Draft,
        symbol_ref: PrimitiveRef::new(library_id, Uuid::now_v7()),
        footprint_ref: None,
        alternate_footprints: Vec::new(),
        sim_ref: None,
        pin_map_overrides: Vec::new(),
        primary_mpn: ManufacturerPart::draft("Yageo", format!("MPN-{n}")),
        alternates: Vec::new(),
        supply: Vec::new(),
        parameters,
        plm: PlmReserved::default(),
        version: "0.0.1".into(),
        released: false,
        symbol_version: "0.0.1".into(),
        footprint_version: String::new(),
        sim_version: String::new(),
        created: now,
        updated: now,
        content_hash: [0u8; 32],
    }
}

/// The Internal PN of rig row `n`.
pub(super) fn part_number(n: usize) -> String {
    format!("R-{n:04}")
}

/// A library at `path` on disk, with `count` rows in [`TABLE`].
pub(super) fn library_on_disk(path: &Path, count: usize) -> Vec<RowId> {
    let name = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let manifest = SnxlibManifest {
        format: FORMAT_TOKEN.into(),
        library_id: Uuid::now_v7(),
        library: LibrarySection {
            name,
            description: None,
        },
        mode: LibraryMode::default(),
        workflow: WorkflowConfig::default(),
        users: UsersConfig::default(),
        classes: Vec::new(),
    };
    let adapter = LocalGitAdapter::init(
        path,
        manifest,
        LibraryInitOptions {
            enable_git: false,
            use_lfs: false,
        },
    )
    .expect("init library");
    let library_id = adapter.library_id();
    let mut ids = Vec::new();
    for n in 0..count {
        let next = row(library_id, n);
        ids.push(RowId::from_uuid(next.row_id));
        adapter.insert_row(TABLE, next, "seed").expect("insert row");
    }
    ids
}

/// Every file under `dir` with its bytes.
pub(super) fn files_under(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).expect("read_dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                walk(&path, out);
            } else {
                let bytes = std::fs::read(&path).expect("read");
                out.insert(path, bytes);
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, &mut out);
    out
}

/// An app with one library mounted and its Browser tab in front.
pub(super) struct LibRig {
    pub app: Signex,
    pub root: PathBuf,
    pub rows: Vec<RowId>,
    pub tmp: TempDir,
}

/// A writable library of `count` rows, its Browser tab open.
pub(super) fn library_rig(count: usize) -> LibRig {
    mounted_rig(count, false)
}

/// The same library mounted read-only.
pub(super) fn read_only_rig(count: usize) -> LibRig {
    mounted_rig(count, true)
}

fn mounted_rig(count: usize, read_only: bool) -> LibRig {
    let tmp = TempDir::new().expect("tempdir");
    let root = tmp.path().join("parts.snxlib");
    let rows = library_on_disk(&root, count);
    let mut app = fresh_app();
    if read_only {
        app.library
            .open_library_read_only(root.clone())
            .expect("mount read-only");
    } else {
        app.library.open_library(root.clone()).expect("mount");
    }
    settle(
        &mut app,
        Message::Library(LibraryMessage::OpenLibraryBrowser(root.clone())),
    );
    LibRig {
        app,
        root,
        rows,
        tmp,
    }
}

impl LibRig {
    /// Send a message and drop its task: nothing it would write runs.
    pub fn send(&mut self, msg: LibraryMessage) {
        let _ = self.app.update(Message::Library(msg));
    }

    /// Send a message and run every task it leads to.
    pub fn run(&mut self, msg: LibraryMessage) {
        settle(&mut self.app, Message::Library(msg));
    }

    pub fn edit(&mut self, msg: BrowserEditMsg) {
        self.run(LibraryMessage::BrowserEdit {
            library_path: self.root.clone(),
            msg,
        });
    }

    pub fn select(&mut self, row_id: RowId) {
        self.run(LibraryMessage::BrowserSelectRow {
            library_path: self.root.clone(),
            table: TABLE.to_string(),
            row_id,
        });
    }

    pub fn open_modal(&mut self, row_id: RowId) {
        self.run(LibraryMessage::BrowserOpenEditModal {
            library_path: self.root.clone(),
            table: TABLE.to_string(),
            row_id,
        });
    }

    pub fn browser(&self) -> &LibraryBrowserState {
        &self.app.library.library_browsers[&self.root]
    }

    pub fn modal(&self) -> &EditRowModalState {
        self.browser()
            .edit_modal
            .as_ref()
            .expect("the modal is open")
    }

    /// The row as the mounted library holds it.
    pub fn row(&self, row_id: RowId) -> Option<ComponentRow> {
        self.app
            .library
            .library_at(&self.root)
            .and_then(|lib| lib.tables.get(TABLE))
            .and_then(|rows| rows.iter().find(|r| RowId::from_uuid(r.row_id) == row_id))
            .cloned()
    }

    pub fn row_count(&self) -> usize {
        self.app
            .library
            .library_at(&self.root)
            .and_then(|lib| lib.tables.get(TABLE))
            .map_or(0, Vec::len)
    }

    pub fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        files_under(self.tmp.path())
    }

    pub fn library_id(&self) -> Uuid {
        self.app
            .library
            .library_at(&self.root)
            .expect("mounted")
            .library_id
    }
}

/// An app with a symbol editor on a saved two-pin `.snxsym`.
pub(super) struct SymbolRig {
    pub app: Signex,
    pub path: PathBuf,
    _tmp: TempDir,
}

/// A symbol editor at its save point, two pins, nothing to undo into.
pub(super) fn symbol_rig() -> SymbolRig {
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("part.snxsym");
    let mut app = fresh_app();
    let file = SymbolFile::from_symbol(Symbol::empty("OPA"));
    app.document_state
        .symbol_editors
        .insert(path.clone(), SymbolDocument::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "OPA".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(path.clone()),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    let mut rig = SymbolRig {
        app,
        path,
        _tmp: tmp,
    };
    for y in [0.0, 2.54] {
        rig.canvas(SymbolEditorMsg::AddPin { x: 0.0, y });
    }
    settle(
        &mut rig.app,
        Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: rig.path.clone(),
            msg: PrimitiveEdit::Save,
        }),
    );
    assert!(!rig.dirty(), "the rig opens at its save point");
    rig
}

impl SymbolRig {
    pub fn canvas(&mut self, msg: SymbolEditorMsg) {
        let _ = self
            .app
            .update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
                path: self.path.clone(),
                msg: PrimitiveEdit::Symbol(msg),
            }));
    }

    pub fn pins(&self) -> &[SymbolPin] {
        let editor = &self.app.document_state.symbol_editors[&self.path];
        &editor.file.symbols[editor.active_idx].pins
    }

    pub fn dirty(&self) -> bool {
        self.app.document_state.is_document_dirty(&self.path)
    }
}

/// Put an empty sheet in a tab of `app`, at its save point and in front.
pub(super) fn open_sheet(app: &mut Signex) -> (PathBuf, TempDir) {
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("audit.snxsch");
    let sheet = SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
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
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path.clone());
    app.interaction_state
        .active_canvas_mut()
        .set_render_cache(Some(
            signex_app::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
        ));
    (path, tmp)
}

/// Project data naming `sheets` in `dir`, with no variants or libraries.
pub(super) fn project_data(dir: &Path, sheets: &[&str]) -> ProjectData {
    ProjectData {
        document_id: None,
        description: None,
        name: "Proj".to_string(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: sheets.first().map(|name| (*name).to_string()),
        pcb_file: None,
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

/// Write `data` to `prj` and load it into `app` as the active project.
pub(super) fn load_project(app: &mut Signex, prj: &Path, data: &ProjectData) -> ProjectId {
    signex_types::project::write_project(prj, data).expect("write project");
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: prj.to_path_buf(),
        data: signex_types::project::parse_project(prj).expect("parse project"),
        pending_libraries: Default::default(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(id);
    id
}
