//! The project on disk and the small drivers the b_project rows share.

use std::path::{Path, PathBuf};

use signex_app::app::{
    ContextMenuMsg, EditMsg, FileMsg, Message, ProjectId, ProjectMsg, ProjectSection,
    ProjectTabMsg, ProjectTabState, ProjectTreeAction, Signex,
};
use signex_types::format::SnxSchematic;
use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::{ChildSheet, FillType, Point, SchematicSheet};
use signex_widgets::tree_view::{TreeKey, TreeNode};
use tempfile::TempDir;
use uuid::Uuid;

use crate::settle_support::settle;

/// An app with `Proj.snxprj` open from disk and its root sheet in front.
pub(super) struct Bench {
    pub app: Signex,
    pub id: ProjectId,
    pub prj: PathBuf,
    pub dir: PathBuf,
    _tmp: TempDir,
}

fn placement(filename: &str, x: f64) -> ChildSheet {
    ChildSheet {
        uuid: Uuid::now_v7(),
        name: filename.trim_end_matches(".snxsch").to_string(),
        filename: filename.to_string(),
        position: Point::new(x, 50.0),
        size: (20.0, 20.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    }
}

fn sheet_placing(children: &str) -> SchematicSheet {
    SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        child_sheets: children
            .split(',')
            .filter(|name| !name.is_empty())
            .enumerate()
            .map(|(i, name)| placement(name, 30.0 * (i as f64 + 1.0)))
            .collect(),
        ..Default::default()
    }
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

fn project_data(dir: &Path, sheets: &[(&str, &str)]) -> ProjectData {
    ProjectData {
        document_id: None,
        description: None,
        name: "Proj".to_string(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: sheets.first().map(|(name, _)| (*name).to_string()),
        pcb_file: None,
        sheets: sheets.iter().map(|(name, _)| entry(name)).collect(),
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

/// Write `Proj.snxprj` and its sheets into `dir`. Each sheet is `(file, the
/// files it places, comma separated)`; a placed file not listed is not written.
pub(super) fn write_project_files(dir: &Path, sheets: &[(&str, &str)]) -> PathBuf {
    for (name, children) in sheets {
        let text = SnxSchematic::new(sheet_placing(children))
            .write_string()
            .expect("encode sheet");
        std::fs::write(dir.join(name), text).expect("write sheet");
    }
    let prj = dir.join("Proj.snxprj");
    signex_types::project::write_project(&prj, &project_data(dir, sheets)).expect("write project");
    prj
}

/// A fresh app with no user overrides.
pub(super) fn fresh_app() -> Signex {
    let _ = signex_app::diagnostics::init_logging();
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    app.ui_state.erc_severity_override.clear();
    app.ui_state.pin_matrix_overrides.clear();
    app.ui_state.setup_profiles.clear();
    app
}

pub(super) fn bench(sheets: &[(&str, &str)]) -> Bench {
    bench_missing(sheets, &[])
}

/// [`bench`] with the files of `missing` gone from disk before the open.
pub(super) fn bench_missing(sheets: &[(&str, &str)], missing: &[&str]) -> Bench {
    let tmp = TempDir::new().expect("tempdir");
    let written = write_project_files(tmp.path(), sheets);
    for name in missing {
        std::fs::remove_file(tmp.path().join(name)).expect("remove file");
    }
    let mut app = fresh_app();
    settle(
        &mut app,
        Message::File(FileMsg::Opened(Some(written.clone()))),
    );
    let root = written.with_file_name(sheets[0].0);
    settle(&mut app, Message::File(FileMsg::Opened(Some(root))));
    // The open may spell the path canonically: every later path follows it.
    let project = app.document_state.projects.first().expect("project open");
    let (id, prj) = (project.id, project.path.clone());
    let dir = prj.parent().expect("project folder").to_path_buf();
    Bench {
        app,
        id,
        prj,
        dir,
        _tmp: tmp,
    }
}

fn key_trail(nodes: &[TreeNode], file: &str, trail: &mut TreeKey) -> bool {
    for node in nodes {
        trail.push(node.key.clone());
        if node.key == file || key_trail(&node.children, file, trail) {
            return true;
        }
        trail.pop();
    }
    false
}

impl Bench {
    pub fn send(&mut self, message: Message) {
        settle(&mut self.app, message);
    }

    pub fn data(&self) -> &ProjectData {
        &self.app.document_state.projects[0].data
    }

    pub fn dirty(&self) -> bool {
        self.app.document_state.is_document_dirty(&self.prj)
    }

    pub fn file(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    pub fn listed(&self) -> Vec<String> {
        self.data().sheet_order()
    }

    pub fn on_disk(&self) -> ProjectData {
        signex_types::project::parse_project(&self.prj).expect("the project file parses")
    }

    /// The loaded sheet whose file is `name`.
    pub fn sheet(&self, name: &str) -> &SchematicSheet {
        let wanted = Some(std::ffi::OsStr::new(name));
        self.app
            .document_state
            .engines
            .iter()
            .find(|(path, _)| path.file_name() == wanted)
            .map(|(_, engine)| engine.document())
            .expect("the sheet is loaded")
    }

    pub fn root_key(&self) -> TreeKey {
        vec![self.id.to_string()]
    }

    /// The tree key of the row whose file is `file`.
    pub fn row(&self, file: &str) -> TreeKey {
        let mut trail = TreeKey::new();
        let tree = &self.app.document_state.panel_ctx.project_tree;
        assert!(key_trail(tree, file, &mut trail), "no tree row for {file}");
        trail
    }

    pub fn node(&self, file: &str) -> &TreeNode {
        let key = self.row(file);
        let mut nodes = self.app.document_state.panel_ctx.project_tree.as_slice();
        let mut found = None;
        for part in &key {
            let node = nodes
                .iter()
                .find(|node| node.key == *part)
                .expect("the keyed row");
            nodes = &node.children;
            found = Some(node);
        }
        found.expect("a non-empty key")
    }

    pub fn action(&mut self, action: ProjectTreeAction) {
        self.send(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
            action,
        )));
    }

    pub fn tab(&mut self, msg: ProjectTabMsg) {
        self.send(Message::Project(ProjectMsg::Tab(msg)));
    }

    /// Project Options on the project, then `section` picked.
    pub fn open_section(&mut self, section: ProjectSection) {
        let root = self.root_key();
        self.action(ProjectTreeAction::OpenProjectOptions(root));
        let project_id = self.id;
        self.tab(ProjectTabMsg::SectionSelected {
            project_id,
            section,
        });
    }

    pub fn tab_state(&self) -> &ProjectTabState {
        self.app
            .document_state
            .project_tabs
            .get(&self.id)
            .expect("the Project tab state")
    }

    pub fn undo(&mut self) {
        self.send(Message::Edit(EditMsg::Undo));
    }

    pub fn redo(&mut self) {
        self.send(Message::Edit(EditMsg::Redo));
    }
}
