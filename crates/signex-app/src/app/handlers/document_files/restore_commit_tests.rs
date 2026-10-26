//! #1025 — a History restore of a footprint or symbol is committed to
//! project git, and the restored bytes stay as they are on disk.

use std::path::{Path, PathBuf};

use signex_library::adapters::local_git_project::LocalGitProjectAdapter;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{FileMsg, Message, Signex, TabInfo, TabKind};

/// Commits `text` as `path` in the project repo and returns the sha.
fn commit_revision(dir: &Path, path: &Path, text: &str) -> String {
    signex_library::git_identity::test_identity::install();
    std::fs::write(path, text).expect("write revision");
    let adapter = LocalGitProjectAdapter::open_or_init(dir.to_path_buf()).expect("repo");
    let rel = path.strip_prefix(dir).expect("rel");
    adapter
        .commit_path(rel, "revision")
        .expect("commit")
        .expect("changed")
        .to_string()
}

fn open_tab(dir: &Path, path: &Path, kind: TabKind) -> Signex {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.document_state.projects[0].data.enable_git = true;
    app.document_state.tabs.push(TabInfo {
        title: "part".into(),
        path: path.to_path_buf(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind,
    });
    app.document_state.active_tab = 0;
    app
}

fn assert_restored_and_committed(app: &Signex, path: &Path, restored: &str) {
    assert_eq!(std::fs::read_to_string(path).expect("read"), restored);
    let pending = &app.document_state.pending_git_commits;
    assert_eq!(pending.len(), 1, "exactly one commit queued");
    assert!(pending[0].message.starts_with("Restore "));
    assert!(!app.document_state.dirty_paths.contains(path));
    assert!(!app.document_state.tabs[0].dirty);
}

fn footprint_text(name: &str) -> String {
    use signex_library::{Footprint, FootprintFile};
    FootprintFile::from_footprint(Footprint::empty(name))
        .to_toml_string()
        .expect("encode")
}

fn symbol_text(name: &str) -> String {
    use signex_library::{Symbol, SymbolFile};
    SymbolFile::from_symbol(Symbol::empty(name))
        .to_toml_string()
        .expect("encode")
}

#[test]
fn restoring_a_footprint_queues_one_restore_commit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path: PathBuf = dir.path().join("part.snxfpt");
    let old = footprint_text("old");
    let sha = commit_revision(dir.path(), &path, &old);
    commit_revision(dir.path(), &path, &footprint_text("new"));
    let mut app = open_tab(dir.path(), &path, TabKind::FootprintEditor(path.clone()));
    app.handle_history_restore_clicked(&sha);
    assert!(app.document_state.footprint_editors.contains_key(&path));
    assert_restored_and_committed(&app, &path, &old);
}

#[test]
fn restoring_a_symbol_queues_one_restore_commit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path: PathBuf = dir.path().join("part.snxsym");
    let old = symbol_text("old");
    let sha = commit_revision(dir.path(), &path, &old);
    commit_revision(dir.path(), &path, &symbol_text("new"));
    let mut app = open_tab(dir.path(), &path, TabKind::SymbolEditor(path.clone()));
    app.handle_history_restore_clicked(&sha);
    assert!(app.document_state.symbol_editors.contains_key(&path));
    assert_restored_and_committed(&app, &path, &old);
}

/// Runs `task` as the iced runtime would and feeds every message back through
/// `update`, following the tasks those return.
fn run_task(app: &mut Signex, task: iced::Task<Message>) {
    use futures::StreamExt;
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return;
    };
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let messages = runtime.block_on(async {
        let mut out = Vec::new();
        while let Some(action) = stream.next().await {
            if let iced_runtime::Action::Output(message) = action {
                out.push(message);
            }
        }
        out
    });
    for message in messages {
        let next = app.update(message);
        run_task(app, next);
    }
}

/// Drains the commit queue and runs every commit against the real repo.
fn run_queued_commits(app: &mut Signex) {
    let task = app.drain_pending_git_commits();
    run_task(app, task);
}

fn log_len(dir: &Path) -> usize {
    let repo = git2::Repository::open(dir).expect("repo");
    let mut walk = repo.revwalk().expect("walk");
    walk.push_head().expect("head");
    walk.count()
}

fn head_subject(dir: &Path) -> String {
    let repo = git2::Repository::open(dir).expect("repo");
    let head = repo.head().expect("head").peel_to_commit().expect("commit");
    let message = head.message().expect("utf-8 message");
    message.lines().next().unwrap_or_default().to_owned()
}

#[test]
fn the_restore_commit_lands_in_git_as_one_restore_commit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("part.snxfpt");
    let sha = commit_revision(dir.path(), &path, &footprint_text("old"));
    commit_revision(dir.path(), &path, &footprint_text("new"));
    let mut app = open_tab(dir.path(), &path, TabKind::FootprintEditor(path.clone()));
    let before = log_len(dir.path());
    app.handle_history_restore_clicked(&sha);
    run_queued_commits(&mut app);
    assert_eq!(log_len(dir.path()), before + 1);
    assert_eq!(head_subject(dir.path()), "Restore part.snxfpt");
    assert!(app.document_state.inflight_git_commits.is_empty());
}

#[test]
fn a_symbol_restore_commit_lands_in_git_as_one_restore_commit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("part.snxsym");
    let sha = commit_revision(dir.path(), &path, &symbol_text("old"));
    commit_revision(dir.path(), &path, &symbol_text("new"));
    let mut app = open_tab(dir.path(), &path, TabKind::SymbolEditor(path.clone()));
    let before = log_len(dir.path());
    app.handle_history_restore_clicked(&sha);
    run_queued_commits(&mut app);
    assert_eq!(log_len(dir.path()), before + 1);
    assert_eq!(head_subject(dir.path()), "Restore part.snxsym");
}

/// A footprint whose stored arc end is off its circle: it loads repaired,
/// so the restored editor differs from disk.
fn off_circle_footprint_text() -> String {
    use signex_library::{Footprint, FootprintFile};
    use signex_sketch::SketchData;
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
    let plane = Plane {
        id: PlaneId::new(),
        kind: PlaneKind::BoardTop,
    };
    let pid = plane.id;
    let mut sketch = SketchData::default();
    sketch.planes.push(plane);
    let mut point = |x, y| {
        let id = SketchEntityId::new();
        sketch
            .entities
            .push(Entity::new(id, pid, EntityKind::Point { x, y }));
        id
    };
    let ids = [point(2.0, 3.0), point(7.0, 3.0), point(2.0, 10.0)];
    sketch.entities.push(Entity::new(
        SketchEntityId::new(),
        pid,
        EntityKind::Arc {
            center: ids[0],
            start: ids[1],
            end: ids[2],
            sweep_ccw: true,
        },
    ));
    let mut fp = Footprint::empty("arc");
    fp.sketch = Some(sketch);
    FootprintFile::from_footprint(fp)
        .to_toml_string()
        .expect("encode")
}

#[test]
fn a_repaired_footprint_restore_stays_dirty_and_the_later_save_is_committed_too() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("part.snxfpt");
    let sha = commit_revision(dir.path(), &path, &off_circle_footprint_text());
    commit_revision(dir.path(), &path, &footprint_text("new"));
    let mut app = open_tab(dir.path(), &path, TabKind::FootprintEditor(path.clone()));
    let before = log_len(dir.path());

    app.handle_history_restore_clicked(&sha);
    assert!(
        app.document_state.tabs[0].dirty,
        "load repair leaves it dirty"
    );
    assert!(app.document_state.dirty_paths.contains(&path));
    assert_eq!(app.document_state.pending_git_commits.len(), 1);
    let queued = app.document_state.pending_git_commits.remove(0);
    assert_eq!(queued.message, "Restore part.snxfpt");

    // The restore commit is now running (key still in flight): it reads the
    // restored bytes before the save below lands.
    let adapter = LocalGitProjectAdapter::open_or_init(dir.path().to_path_buf()).expect("repo");
    adapter
        .commit_path(&queued.rel_path, &queued.message)
        .expect("commit");
    let save = app.update(Message::File(FileMsg::Save));
    run_task(&mut app, save);
    assert!(
        app.document_state.pending_git_commits.is_empty(),
        "re-queued, not enqueued"
    );

    // Its completion releases exactly one more commit: the save.
    let done = app.handle_project_git_commit_done(
        queued.project_root,
        queued.rel_path,
        Ok(Some("x".into())),
    );
    run_task(&mut app, done);
    assert_eq!(log_len(dir.path()), before + 2);
    assert!(head_subject(dir.path()).starts_with("Save footprint"));
    assert!(app.document_state.inflight_git_commits.is_empty());
}

fn assert_failed_reload_blocks_save(
    app: &mut Signex,
    dir: &Path,
    path: &Path,
    restored: &str,
    log_before: usize,
) {
    let card = app
        .document_state
        .error_notice
        .as_ref()
        .expect("restore card");
    let card = format!("{card:?}");
    assert!(card.contains("not committed to version control"), "{card}");
    assert!(card.contains("restore a different revision"), "{card}");
    assert!(!app.document_state.footprint_editors.contains_key(path));
    assert!(!app.document_state.symbol_editors.contains_key(path));
    assert!(!app.document_state.engines.contains_key(path));
    assert!(app.document_state.pending_git_commits.is_empty());
    assert!(app.document_state.inflight_git_commits.is_empty());
    assert!(!app.document_state.dirty_paths.contains(path));
    assert!(!app.document_state.tabs[0].dirty);
    // Ctrl+S, then the write task as the iced runtime runs it.
    let save = app.update(Message::File(FileMsg::Save));
    run_task(app, save);
    assert_eq!(std::fs::read_to_string(path).expect("read"), restored);
    assert!(app.document_state.pending_git_commits.is_empty());
    assert_eq!(log_len(dir), log_before, "no restore commit");
}

#[test]
fn a_footprint_restore_that_fails_to_reload_keeps_the_restored_bytes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("part.snxfpt");
    let sha = commit_revision(dir.path(), &path, "not a footprint");
    commit_revision(dir.path(), &path, &footprint_text("new"));
    let mut app = open_tab(dir.path(), &path, TabKind::FootprintEditor(path.clone()));
    let file =
        signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("new"));
    app.document_state.footprint_editors.insert(
        path.clone(),
        crate::app::FootprintEditorState::new(path.clone(), file),
    );
    let before = log_len(dir.path());
    app.handle_history_restore_clicked(&sha);
    assert_failed_reload_blocks_save(&mut app, dir.path(), &path, "not a footprint", before);
}

#[test]
fn a_symbol_restore_that_fails_to_reload_keeps_the_restored_bytes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("part.snxsym");
    let sha = commit_revision(dir.path(), &path, "not a symbol");
    commit_revision(dir.path(), &path, &symbol_text("new"));
    let mut app = open_tab(dir.path(), &path, TabKind::SymbolEditor(path.clone()));
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("new"));
    app.document_state.symbol_editors.insert(
        path.clone(),
        crate::app::SymbolEditorState::new(path.clone(), file),
    );
    let before = log_len(dir.path());
    app.handle_history_restore_clicked(&sha);
    assert_failed_reload_blocks_save(&mut app, dir.path(), &path, "not a symbol", before);
}

#[test]
fn a_schematic_restore_that_fails_to_reload_keeps_the_restored_bytes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("a.snxsch");
    let sha = commit_revision(dir.path(), &path, "not a schematic");
    commit_revision(dir.path(), &path, "newer revision");
    let mut app = open_tab(dir.path(), &path, TabKind::Schematic);
    let mut engine =
        signex_engine::Engine::new(signex_types::schematic::SchematicSheet::default()).unwrap();
    engine.set_path(Some(path.clone()));
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.active_path = Some(path.clone());
    let before = log_len(dir.path());
    app.handle_history_restore_clicked(&sha);
    assert_failed_reload_blocks_save(&mut app, dir.path(), &path, "not a schematic", before);
}

#[test]
fn a_schematic_restore_that_cannot_be_read_says_to_reopen_and_saves_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("a.snxsch");
    commit_revision(dir.path(), &path, "a revision");
    let mut app = open_tab(dir.path(), &path, TabKind::Schematic);
    let mut engine =
        signex_engine::Engine::new(signex_types::schematic::SchematicSheet::default()).unwrap();
    engine.set_path(Some(path.clone()));
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.active_path = Some(path.clone());
    // A directory where the file was: reading it fails with an IO error.
    std::fs::remove_file(&path).expect("remove");
    std::fs::create_dir(&path).expect("dir");
    app.reload_active_tab_from_disk();
    let card = format!(
        "{:?}",
        app.document_state.error_notice.as_ref().expect("card")
    );
    assert!(
        card.contains("Close and reopen the tab once the file can be read"),
        "{card}"
    );
    assert!(!app.document_state.engines.contains_key(&path));
    let save = app.update(Message::File(FileMsg::Save));
    run_task(&mut app, save);
    assert!(path.is_dir(), "Ctrl+S wrote nothing over the path");
    assert!(app.document_state.pending_git_commits.is_empty());
}

#[test]
fn a_restore_queues_nothing_when_project_git_is_off() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("part.snxfpt");
    let sha = commit_revision(dir.path(), &path, &footprint_text("old"));
    commit_revision(dir.path(), &path, &footprint_text("new"));
    let mut app = open_tab(dir.path(), &path, TabKind::FootprintEditor(path.clone()));
    app.document_state.projects[0].data.enable_git = false;
    app.handle_history_restore_clicked(&sha);
    assert!(app.document_state.pending_git_commits.is_empty());
    assert!(app.document_state.inflight_git_commits.is_empty());
}

#[test]
fn enqueueing_a_file_twice_while_inflight_runs_exactly_one_more_commit_with_the_newest_message() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("a.snxsch");
    let mut app = open_tab(dir.path(), &path, TabKind::Schematic);
    app.commit_save_to_project_git(&path, "first");
    // Running now: out of the queue, key still in flight.
    let running = app.document_state.pending_git_commits.remove(0);
    app.commit_save_to_project_git(&path, "second");
    app.commit_save_to_project_git(&path, "third");
    assert!(app.document_state.pending_git_commits.is_empty());
    let key = (running.project_root.clone(), running.rel_path.clone());
    let _ = app.handle_project_git_commit_done(
        running.project_root.clone(),
        running.rel_path.clone(),
        Ok(None),
    );
    // The released commit was drained into a task and is the one now in flight.
    assert!(app.document_state.requeued_git_commits.is_empty());
    assert!(app.document_state.inflight_git_commits.contains(&key));
    assert!(
        app.document_state
            .active_git_repos
            .contains(&running.project_root)
    );
    let _ = app.handle_project_git_commit_done(running.project_root, running.rel_path, Ok(None));
    assert!(app.document_state.inflight_git_commits.is_empty());
    assert!(app.document_state.pending_git_commits.is_empty());
}

#[test]
fn the_requeued_commit_carries_the_newest_message_and_runs_after_an_err() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("a.snxsch");
    let mut app = open_tab(dir.path(), &path, TabKind::Schematic);
    app.commit_save_to_project_git(&path, "first");
    let running = app.document_state.pending_git_commits.remove(0);
    app.commit_save_to_project_git(&path, "second");
    app.commit_save_to_project_git(&path, "third");
    // The running commit failed: the re-queued one still has to run.
    let _ = app.handle_project_git_commit_done(
        running.project_root.clone(),
        running.rel_path.clone(),
        Err(crate::app::GitCommitFailure::Failed("boom".into())),
    );
    assert!(app.document_state.requeued_git_commits.is_empty());
    assert!(
        app.document_state
            .inflight_git_commits
            .contains(&(running.project_root, running.rel_path))
    );
}

#[test]
fn a_queued_save_followed_by_a_restore_is_one_restore_commit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("a.snxsch");
    let mut app = open_tab(dir.path(), &path, TabKind::Schematic);
    app.commit_save_to_project_git(&path, "Save a.snxsch");
    app.commit_save_to_project_git(&path, "Restore a.snxsch");
    let pending = &app.document_state.pending_git_commits;
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].message, "Restore a.snxsch");
    assert!(app.document_state.requeued_git_commits.is_empty());
}
