//! #859 — what a save completion does to git and to a parked engine.

use std::path::{Path, PathBuf};

use super::restore_commit_tests::restore;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::WindowKind;
use crate::app::{FileMsg, Message, Signex};
use crate::persist::{DocumentSaveDone, SaveOutcome};

fn app_with_engine(dir: &Path) -> (Signex, PathBuf, u64) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.document_state.projects[0].data.enable_git = true;
    let path = dir.join("a.snxsch");
    let sheet = signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        ..Default::default()
    };
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    let revision = engine.revision();
    app.document_state.engines.insert(path.clone(), engine);
    (app, path, revision)
}

fn land(app: &mut Signex, path: &Path, revision: u64) {
    let _ = app.update(Message::File(FileMsg::DocumentSaved(DocumentSaveDone {
        path: path.to_path_buf(),
        revision,
        net_color_revision: 0,
        save_as_from: None,
        bytes: std::sync::Arc::from(&b"saved"[..]),
        result: Ok(SaveOutcome::Written),
        recovery_hold: None,
    })));
}

#[test]
fn a_completion_for_the_current_revision_commits_to_git() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path, revision) = app_with_engine(dir.path());
    land(&mut app, &path, revision);
    assert_eq!(app.document_state.inflight_git_commits.len(), 1);
}

#[test]
fn a_completion_for_another_revision_makes_no_save_commit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path, revision) = app_with_engine(dir.path());
    land(&mut app, &path, revision.wrapping_add(1));
    assert!(app.document_state.inflight_git_commits.is_empty());
    assert!(app.document_state.pending_git_commits.is_empty());
}

#[test]
fn a_clean_engine_is_dropped_unless_an_undocked_window_holds_the_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path, revision) = app_with_engine(dir.path());
    app.ui_state.windows.insert(
        iced::window::Id::unique(),
        WindowKind::UndockedTab {
            path: path.clone(),
            title: "a".into(),
        },
    );
    land(&mut app, &path, revision);
    assert!(app.document_state.engines.contains_key(&path));

    app.ui_state.windows.clear();
    land(&mut app, &path, revision);
    assert!(!app.document_state.engines.contains_key(&path));
}

#[test]
fn a_pcb_completion_requests_the_project_git_commit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_pcb(dir.path());
    let revision = app.document_state.pcb_revisions[&path];
    land(&mut app, &path, revision);
    assert_eq!(app.document_state.inflight_git_commits.len(), 1);
}

#[test]
fn a_pcb_completion_commits_even_when_its_tab_closed_mid_write() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_pcb(dir.path());
    let revision = app.document_state.pcb_revisions[&path];
    app.document_state.tabs.clear();
    land(&mut app, &path, revision);
    assert_eq!(app.document_state.inflight_git_commits.len(), 1);
}

#[test]
fn a_write_from_a_closed_tab_does_not_clean_the_reopened_one() {
    let dir = tempfile::tempdir().expect("tempdir");
    signex_library::git_identity::test_identity::install();
    let (mut app, path) = app_with_pcb(dir.path());
    mark_dirty(&mut app, &path);
    let landed = begin_save(&mut app);
    let revision_a = app.document_state.pcb_revisions[&path];
    app.document_state.tabs.clear();
    app.open_pcb_tab(path.clone(), "board".into(), empty_board());
    mark_dirty(&mut app, &path);
    // Edit the reopened tab as many times as A had edits; a per-tab counter
    // would now have reached A's revision.
    for _ in 0..revision_a {
        let board = empty_board_with(&app);
        app.document_state.set_pcb_board(0, board);
    }
    assert_ne!(app.document_state.pcb_revisions[&path], revision_a);
    for message in landed {
        let _ = app.update(message);
    }
    assert!(app.document_state.tabs[0].dirty);
    assert!(app.document_state.dirty_paths.contains(&path));
    assert!(no_commit_requested(&app));
}

#[test]
fn a_renamed_pcb_tab_still_saves_clean_and_commits() {
    let dir = tempfile::tempdir().expect("tempdir");
    signex_library::git_identity::test_identity::install();
    let (mut app, path) = app_with_pcb(dir.path());
    let renamed = dir.path().join("renamed.snxpcb");
    app.rekey_open_document(&path, &renamed);
    assert!(app.document_state.pcb_revisions.contains_key(&renamed));
    mark_dirty(&mut app, &renamed);
    press_save(&mut app);
    assert!(!app.document_state.tabs[0].dirty);
    assert!(!app.document_state.dirty_paths.contains(&renamed));
    assert_eq!(app.document_state.inflight_git_commits.len(), 1);
}

#[test]
fn a_pcb_tab_with_no_revision_entry_still_saves_clean() {
    let dir = tempfile::tempdir().expect("tempdir");
    signex_library::git_identity::test_identity::install();
    let (mut app, path) = app_with_pcb(dir.path());
    app.document_state.pcb_revisions.remove(&path);
    mark_dirty(&mut app, &path);
    press_save(&mut app);
    assert!(!app.document_state.tabs[0].dirty);
    assert_eq!(app.document_state.inflight_git_commits.len(), 1);
}

#[test]
fn a_restored_or_dropped_board_gets_a_fresh_revision() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_pcb(dir.path());
    let opened = app.document_state.pcb_revisions[&path];
    let board = empty_board_with(&app);
    app.document_state.set_pcb_board(0, board);
    let restored = app.document_state.pcb_revisions[&path];
    assert!(restored > opened);
    app.document_state.drop_pcb_board(0);
    assert!(app.document_state.pcb_revisions[&path] > restored);
}

fn app_with_pcb(dir: &Path) -> (Signex, PathBuf) {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.document_state.projects[0].data.enable_git = true;
    let path = dir.join("board.snxpcb");
    app.open_pcb_tab(path.clone(), "board".into(), empty_board());
    (app, path)
}

fn mark_dirty(app: &mut Signex, path: &Path) {
    app.document_state.tabs[0].dirty = true;
    app.document_state.dirty_paths.insert(path.to_path_buf());
}

/// Ctrl+S through `update`, then the write task as the iced runtime runs it.
fn press_save(app: &mut Signex) {
    for message in begin_save(app) {
        let _ = app.update(message);
    }
}

/// Ctrl+S and the write itself; the completion messages are returned unrouted
/// so a test can edit the document before they land.
fn begin_save(app: &mut Signex) -> Vec<Message> {
    use futures::StreamExt;
    let task = app.update(Message::File(FileMsg::Save));
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return Vec::new();
    };
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    runtime.block_on(async {
        let mut out = Vec::new();
        while let Some(action) = stream.next().await {
            if let iced_runtime::Action::Output(message) = action {
                out.push(message);
            }
        }
        out
    })
}

fn no_commit_requested(app: &Signex) -> bool {
    app.document_state.inflight_git_commits.is_empty()
        && app.document_state.pending_git_commits.is_empty()
}

#[test]
fn ctrl_s_on_a_dirty_pcb_writes_the_board_and_queues_the_commit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_pcb(dir.path());
    mark_dirty(&mut app, &path);
    press_save(&mut app);
    assert_eq!(
        std::fs::read(&path).expect("board written"),
        encoded(&empty_board_with(&app))
    );
    assert_eq!(app.document_state.inflight_git_commits.len(), 1);
    assert!(!app.document_state.dirty_paths.contains(&path));
}

#[test]
fn an_edit_during_a_pcb_write_stays_dirty_and_queues_no_stale_commit() {
    let dir = tempfile::tempdir().expect("tempdir");
    signex_library::git_identity::test_identity::install();
    let (mut app, path) = app_with_pcb(dir.path());
    mark_dirty(&mut app, &path);
    let landed = begin_save(&mut app);
    let mut edited = empty_board_with(&app);
    edited.thickness = 2.0;
    app.document_state.set_pcb_board(0, edited);
    for message in landed {
        let _ = app.update(message);
    }
    assert!(app.document_state.tabs[0].dirty);
    assert!(app.document_state.dirty_paths.contains(&path));
    assert!(no_commit_requested(&app));
    press_save(&mut app);
    assert!(!app.document_state.dirty_paths.contains(&path));
    assert_eq!(app.document_state.inflight_git_commits.len(), 1);
}

#[test]
fn a_failed_pcb_write_stays_dirty_queues_no_commit_and_reports() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_pcb(dir.path());
    // A directory where the board belongs makes the atomic rename fail.
    std::fs::create_dir(&path).expect("block the target");
    mark_dirty(&mut app, &path);
    press_save(&mut app);
    assert!(path.is_dir(), "nothing replaced the blocking directory");
    assert!(app.document_state.dirty_paths.contains(&path));
    assert!(app.document_state.tabs[0].dirty);
    assert!(no_commit_requested(&app));
    assert!(app.document_state.error_notice.is_some());
    let path_text = path.display().to_string();
    assert!(
        app.document_state
            .panel_ctx
            .diagnostics
            .iter()
            .any(|entry| entry.message.contains(&path_text)),
        "the failure must reach the Messages panel"
    );
}

#[test]
fn ctrl_s_on_a_clean_pcb_leaves_the_file_another_tool_changed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_pcb(dir.path());
    std::fs::write(&path, "changed by git pull").expect("external write");
    press_save(&mut app);
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "changed by git pull"
    );
    assert!(no_commit_requested(&app));
    assert!(app.document_state.error_notice.is_none());
}

#[test]
fn ctrl_s_after_a_restore_that_cannot_be_re_read_refuses_to_undo_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_pcb(dir.path());
    std::fs::write(&path, "restored but unparsable").expect("restore");
    app.document_state.dirty_paths.insert(path.clone());
    app.reload_active_tab_from_disk();
    assert!(app.document_state.error_notice.is_some(), "restore card");
    assert!(
        matches!(app.begin_pcb_save(path.clone()), Ok(None)),
        "clean tab: nothing to write"
    );
    press_save(&mut app);
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "restored but unparsable"
    );
    assert!(no_commit_requested(&app));
}

#[test]
fn ctrl_s_on_a_newer_minor_board_writes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    app.document_state.projects[0].data.enable_git = true;
    let path = dir.path().join("board.snxpcb");
    let mut board = empty_board();
    board.unknown.newer_minor_source = Some("snxpcb/1.9".into());
    app.open_pcb_tab(path.clone(), "board".into(), board);
    mark_dirty(&mut app, &path);
    press_save(&mut app);
    assert!(!path.exists());
    assert!(no_commit_requested(&app));
    let notice = app.document_state.error_notice.as_ref().expect("notice");
    assert!(notice.detail.contains("read-only (newer format)"));
}

/// The board the first tab holds, as `begin_pcb_save` encodes it.
fn empty_board_with(app: &Signex) -> signex_types::pcb::PcbBoard {
    app.document_state.tabs[0]
        .cached_document
        .as_ref()
        .and_then(|doc| doc.as_pcb())
        .expect("pcb tab")
        .clone()
}

fn encoded(board: &signex_types::pcb::PcbBoard) -> Vec<u8> {
    signex_types::format::SnxPcb::new(board.clone())
        .write_string()
        .expect("encode")
        .into_bytes()
}

/// Commits `bytes` as `path` in the project repo and returns the sha.
fn commit_revision(dir: &Path, path: &Path, bytes: &[u8]) -> String {
    use signex_library::adapters::local_git_project::LocalGitProjectAdapter;
    signex_library::git_identity::test_identity::install();
    std::fs::write(path, bytes).expect("write revision");
    let adapter = LocalGitProjectAdapter::open_or_init(dir.to_path_buf()).expect("repo");
    let rel = path.strip_prefix(dir).expect("rel");
    adapter
        .commit_path(rel, "revision")
        .expect("commit")
        .expect("changed")
        .to_string()
}

#[test]
fn a_restore_that_reads_as_non_utf8_refuses_to_undo_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_pcb(dir.path());
    let old = commit_revision(dir.path(), &path, &[0xff, 0xfe, 0x00, 0x80]);
    commit_revision(dir.path(), &path, &encoded(&empty_board()));
    restore(&mut app, &old);
    assert!(app.document_state.error_notice.is_some(), "restore card");
    assert!(no_commit_requested(&app));
    assert!(
        app.interaction_state
            .pcb_canvas
            .active_renderer_snapshot()
            .is_none()
    );
    assert!(!app.document_state.dirty_paths.contains(&path));
    assert!(
        matches!(app.begin_pcb_save(path.clone()), Ok(None)),
        "clean tab: nothing to write"
    );
    press_save(&mut app);
    assert_eq!(
        std::fs::read(&path).expect("read"),
        vec![0xff, 0xfe, 0x00, 0x80]
    );
}

#[test]
fn closing_a_pcb_tab_drops_its_dirty_marker() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_pcb(dir.path());
    mark_dirty(&mut app, &path);
    assert!(app.document_state.dirty_paths.contains(&path));
    let _ = app.close_tab_now(0);
    assert!(!app.document_state.dirty_paths.contains(&path));
}

#[test]
fn standard_pcb_files_count_as_pcb_paths() {
    assert!(super::save_async::is_pcb_path(Path::new("x.standard_pcb")));
}

#[test]
fn ctrl_s_after_a_successful_restore_keeps_the_restored_bytes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, path) = app_with_pcb(dir.path());
    // Trailing blank lines survive a restore but not a re-encode.
    let mut restored = encoded(&empty_board());
    restored.extend_from_slice(
        b"

",
    );
    let old = commit_revision(dir.path(), &path, &restored);
    commit_revision(dir.path(), &path, &encoded(&empty_board()));
    restore(&mut app, &old);
    assert!(app.document_state.error_notice.is_none());
    assert!(!app.document_state.dirty_paths.contains(&path));
    assert_eq!(app.document_state.pending_git_commits.len(), 1);
    assert!(
        app.document_state.pending_git_commits[0]
            .message
            .starts_with("Restore ")
    );
    press_save(&mut app);
    assert_eq!(std::fs::read(&path).expect("read"), restored);
    // The commit task ran inside `press_save`; Ctrl+S added no second one.
    assert!(no_commit_requested(&app));
    let log = std::process::Command::new("git")
        .arg("-C")
        .arg(dir.path())
        .args(["log", "--format=%s"])
        .output()
        .expect("git log");
    let log = String::from_utf8_lossy(&log.stdout);
    assert_eq!(log.lines().filter(|l| l.starts_with("Restore ")).count(), 1);
    assert_eq!(log.lines().count(), 3, "two revisions plus the restore");
}

pub(super) fn empty_board() -> signex_types::pcb::PcbBoard {
    signex_types::pcb::PcbBoard {
        unknown: Default::default(),
        uuid: uuid::Uuid::new_v4(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: vec![],
        layers: vec![],
        setup: None,
        nets: vec![],
        footprints: vec![],
        segments: vec![],
        vias: vec![],
        zones: vec![],
        graphics: vec![],
        texts: vec![],
    }
}
