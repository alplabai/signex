//! #859 — synchronous disk writers wait for an in-flight document write.
//!
//! A sheet rename moves the file and a History restore rewrites it. A save
//! still running for that path must finish first, or it lands afterwards
//! and resurrects the old file / reverts the restored content.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::StreamExt;
use signex_app::app::{
    LoadedProject, Message, RemoveChoice, RemoveDialogState, RemoveMsg, RenameDialogState,
    RenameMsg, Signex, TabInfo, TabKind,
};
use signex_app::dock::DockMessage;
use signex_app::panels::PanelMsg;
use signex_app::persist::{DocumentSaves, DocumentSink};
use signex_library::adapters::local_git_project::LocalGitProjectAdapter;
use signex_types::project::ProjectData;
use signex_types::schematic::SchematicSheet;
use tempfile::TempDir;

/// Writes the real file, but only after the test lets it.
struct GatedFileSink {
    entered: Mutex<mpsc::Sender<()>>,
    proceed: Mutex<mpsc::Receiver<()>>,
}

impl DocumentSink for GatedFileSink {
    fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        self.entered.lock().unwrap().send(()).unwrap();
        self.proceed.lock().unwrap().recv().unwrap();
        std::fs::write(path, bytes)
    }
}

fn sheet(paper: &str) -> SchematicSheet {
    SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: paper.to_string(),
        ..Default::default()
    }
}

fn sheet_text(paper: &str) -> String {
    signex_types::format::SnxSchematic::new(sheet(paper))
        .write_string()
        .unwrap()
}

/// App with a project at `dir` and a dirty schematic open at `path`, whose
/// saves go through a gated sink.
fn app_with_gated_sheet(dir: &Path, path: &Path) -> (Signex, mpsc::Receiver<()>, mpsc::Sender<()>) {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (proceed_tx, proceed_rx) = mpsc::channel();
    let (mut app, _boot) = Signex::new();
    app.document_state.document_saves = DocumentSaves::new(Arc::new(GatedFileSink {
        entered: Mutex::new(entered_tx),
        proceed: Mutex::new(proceed_rx),
    }));
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: dir.join("P.snxprj"),
        data: ProjectData {
            name: "P".into(),
            dir: dir.to_string_lossy().into_owned(),
            schematic_root: None,
            pcb_file: None,
            sheets: Vec::new(),
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            unknown: Default::default(),
        },
        pending_libraries: Default::default(),
        case_fold: Default::default(),
        case_probed: false,
    });
    app.document_state.active_project = Some(id);
    let mut engine = signex_engine::Engine::new(sheet("A3")).unwrap();
    engine.set_path(Some(path.to_path_buf()));
    app.document_state
        .engines
        .insert(path.to_path_buf(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "doc".into(),
        path: path.to_path_buf(),
        cached_document: None,
        dirty: true,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.to_path_buf());
    app.document_state.dirty_paths.insert(path.to_path_buf());
    (app, entered_rx, proceed_tx)
}

fn run(task: iced::Task<Message>) -> Vec<Message> {
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return Vec::new();
    };
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let mut out = Vec::new();
        while let Some(action) = stream.next().await {
            if let iced_runtime::Action::Output(message) = action {
                out.push(message);
            }
        }
        out
    })
}

/// Start a Save, wait until its write is inside the sink, and release the
/// write 300 ms from now. Returns the thread that drives the save task.
fn start_save_released_later(
    app: &mut Signex,
    entered: &mpsc::Receiver<()>,
    proceed: mpsc::Sender<()>,
) -> std::thread::JoinHandle<Vec<Message>> {
    let save = app.update(Message::File(signex_app::app::FileMsg::Save));
    let driver = std::thread::spawn(move || run(save));
    entered
        .recv_timeout(Duration::from_secs(5))
        .expect("the write is inside the sink");
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        proceed.send(()).unwrap();
    });
    driver
}

#[test]
fn renaming_a_sheet_waits_for_its_in_flight_save_and_does_not_resurrect_the_old_file() {
    let tmp = TempDir::new().unwrap();
    let old: PathBuf = tmp.path().join("Power.snxsch");
    let new: PathBuf = tmp.path().join("PSU.snxsch");
    std::fs::write(&old, b"on disk").unwrap();
    let (mut app, entered, proceed) = app_with_gated_sheet(tmp.path(), &old);

    let driver = start_save_released_later(&mut app, &entered, proceed);
    app.ui_state.rename_dialog = Some(RenameDialogState {
        project_id: None,
        target_path: old.clone(),
        tree_path: vec![0],
        buffer: "PSU.snxsch".to_string(),
        error: None,
        is_project_rename: false,
    });
    crate::settle_support::settle(&mut app, Message::Rename(RenameMsg::Submit));
    for message in driver.join().unwrap() {
        let _ = app.update(message);
    }

    assert!(!old.exists(), "the in-flight write recreated the old name");
    assert!(new.exists());
    assert!(
        !app.document_state.engines.contains_key(&old),
        "the old key must not be revived by the completion"
    );
}

#[test]
fn a_history_restore_waits_for_the_in_flight_save_so_the_restored_content_stays() {
    signex_library::git_identity::test_identity::install();
    let tmp = TempDir::new().unwrap();
    let path: PathBuf = tmp.path().join("Power.snxsch");
    let restored = sheet_text("A4");
    std::fs::write(&path, &restored).unwrap();
    let adapter = LocalGitProjectAdapter::open_or_init(tmp.path().to_path_buf()).unwrap();
    let sha = adapter
        .commit_path(Path::new("Power.snxsch"), "add sheet")
        .unwrap()
        .expect("the sheet changed, so a commit is made")
        .to_string();
    let (mut app, entered, proceed) = app_with_gated_sheet(tmp.path(), &path);

    let driver = start_save_released_later(&mut app, &entered, proceed);
    // A dirty sheet makes Restore a two-click confirm; click twice.
    for _ in 0..2 {
        let _ = app.update(Message::Dock(DockMessage::Panel(
            PanelMsg::HistoryRestoreClicked { sha: sha.clone() },
        )));
    }
    for message in driver.join().unwrap() {
        let _ = app.update(message);
    }

    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        restored,
        "the old write reverted the restored content"
    );
    assert!(
        app.document_state.dirty_paths.contains(&path),
        "the restored engine is not what the stale snapshot wrote"
    );
}

#[test]
fn deleting_a_file_waits_for_its_in_flight_save_and_it_stays_deleted() {
    let tmp = TempDir::new().unwrap();
    let path: PathBuf = tmp.path().join("Power.snxsch");
    std::fs::write(&path, b"on disk").unwrap();
    let (mut app, entered, proceed) = app_with_gated_sheet(tmp.path(), &path);

    let driver = start_save_released_later(&mut app, &entered, proceed);
    app.ui_state.remove_dialog = Some(RemoveDialogState {
        target_path: path.clone(),
        tree_path: vec![0],
        project_id: None,
        display_name: "Power.snxsch".to_string(),
        is_outside_project: false,
    });
    crate::settle_support::settle(
        &mut app,
        Message::Remove(RemoveMsg::Confirm(RemoveChoice::DeleteFile)),
    );
    for message in driver.join().unwrap() {
        let _ = app.update(message);
    }

    assert!(
        !path.exists(),
        "the in-flight write recreated the deleted file"
    );
    assert!(!app.document_state.engines.contains_key(&path));
}
