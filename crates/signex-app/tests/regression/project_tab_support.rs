//! Shared fixture for the Project tab regressions (#1202): a saved project
//! loaded the way project open leaves it, and the messages the tab sends.

use signex_app::app::{
    ContextMenuMsg, FileMsg, LoadedProject, Message, ProjectMsg, ProjectTabMsg, ProjectTreeAction,
    Signex, TabInfo, TabKind,
};
use signex_app::dock::DockMessage;
use signex_app::panels::{NetClassMsg, PanelMsg};
use signex_types::project::{ProjectData, parse_project, write_project};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

pub struct Fixture {
    pub app: Signex,
    pub prj: PathBuf,
    pub tmp: TempDir,
}

/// A saved project `Oscar.snxprj` (root sheet + PCB named, one library),
/// loaded the way project open leaves it.
pub fn fixture() -> Fixture {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    let prj = dir.join("Oscar.snxprj");
    let data = ProjectData {
        name: "Oscar".to_string(),
        description: None,
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: Some("Oscar.snxsch".to_string()),
        pcb_file: Some("Oscar.snxpcb".to_string()),
        sheets: Vec::new(),
        variant_definitions: Vec::new(),
        active_variant: None,
        libraries: vec![signex_types::project::LibraryEntry {
            path: PathBuf::from("parts.snxlib"),
            kind: signex_types::project::LibraryEntryKind::ProjectLocal,
            library_id: None,
            unknown: Default::default(),
        }],
        enable_git: false,
        net_classes: Vec::new(),
        rules: None,
        annotation: None,
        unknown: Default::default(),
    };
    write_project(&prj, &data).unwrap();
    let (mut app, _task) = Signex::new();
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: prj.clone(),
        data: parse_project(&prj).unwrap(),
        pending_libraries: Default::default(),
        case_decided: false,
        case_fold: Default::default(),
        case_probed: false,
    });
    app.document_state.active_project = Some(id);
    Fixture { app, prj, tmp }
}

pub fn open_tab(app: &mut Signex) {
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
        ProjectTreeAction::OpenProjectOptions(vec![0]),
    )));
}

pub fn project_id(app: &Signex) -> signex_app::app::ProjectId {
    app.document_state.projects[0].id
}

pub fn edit_description(app: &mut Signex, text: &str) {
    let project_id = project_id(app);
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::DescriptionEdited {
            project_id,
            text: text.to_string(),
        },
    )));
}

pub fn add_net_class(app: &mut Signex) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::NetClass(
        NetClassMsg::Add,
    ))));
}

pub fn save(app: &mut Signex) {
    crate::settle_support::settle(app, Message::File(FileMsg::Save));
}

pub fn on_disk(path: &Path) -> ProjectData {
    parse_project(path).unwrap()
}

pub fn is_dirty(f: &Fixture) -> bool {
    let path = &f.app.document_state.projects[0].path;
    f.app.document_state.dirty_paths.contains(path)
}

/// Open the root sheet `Oscar.snxsch` as a dirty schematic tab of the
/// project and make it the active tab. Returns its path.
pub fn open_dirty_root_sheet(f: &mut Fixture) -> PathBuf {
    let path = f.tmp.path().join("Oscar.snxsch");
    let mut engine = signex_engine::Engine::new(signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        ..Default::default()
    })
    .expect("engine");
    engine.set_path(Some(path.clone()));
    let project_id = project_id(&f.app);
    let document = &mut f.app.document_state;
    document.engines.insert(path.clone(), engine);
    document.tabs.push(TabInfo {
        title: "Oscar".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: true,
        project_id: Some(project_id),
        kind: TabKind::Schematic,
    });
    document.dirty_paths.insert(path.clone());
    // What a tab switch to the sheet leaves behind.
    document.active_tab = document.tabs.len() - 1;
    document.active_path = Some(path.clone());
    path
}

/// Run `body` on its own thread and fail if it does not finish: an `update`
/// that blocks on a write which only runs after `update` returns would hang
/// the app, and here hangs the test instead.
pub fn within_watchdog(what: &str, body: impl FnOnce() + Send + 'static) {
    use std::sync::mpsc::{RecvTimeoutError, channel};
    let (done, finished) = channel();
    let worker = std::thread::spawn(move || {
        body();
        let _ = done.send(());
    });
    match finished.recv_timeout(std::time::Duration::from_secs(60)) {
        Ok(()) => worker.join().expect("watched body"),
        Err(RecvTimeoutError::Disconnected) => {
            if let Err(panic) = worker.join() {
                std::panic::resume_unwind(panic);
            }
        }
        Err(RecvTimeoutError::Timeout) => panic!("{what}: update blocked (no answer in 60 s)"),
    }
}
