//! #1025 — a History restore of a footprint or symbol is committed to
//! project git, and the restored bytes stay as they are on disk.

use std::path::{Path, PathBuf};

use signex_library::adapters::local_git_project::LocalGitProjectAdapter;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{FileMsg, Message, Signex, TabInfo, TabKind};

/// Commits `text` as `path` in the project repo and returns the sha.
pub(super) fn commit_revision(dir: &Path, path: &Path, text: &str) -> String {
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

pub(super) fn open_tab(dir: &Path, path: &Path, kind: TabKind) -> Signex {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.document_state.projects[0].data.enable_git = true;
    app.document_state.tabs.push(TabInfo {
        title: "part".into(),
        path: path.to_path_buf(),
        cached_document: None,
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
    assert!(!app.document_state.is_document_dirty(path));
    assert!(
        !app.document_state
            .is_document_dirty(&app.document_state.tabs[0].path)
    );
}

pub(super) fn footprint_text(name: &str) -> String {
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
    restore(&mut app, &sha);
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
    restore(&mut app, &sha);
    assert!(app.document_state.symbol_editors.contains_key(&path));
    assert_restored_and_committed(&app, &path, &old);
}

/// Runs `task` as the iced runtime would and returns the messages it emits,
/// without applying them.
pub(crate) fn collect_messages(task: iced::Task<Message>) -> Vec<Message> {
    use futures::StreamExt;
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

/// Runs `task` and feeds every message back through `update`, following the
/// tasks those return.
pub(super) fn run_task(app: &mut Signex, task: iced::Task<Message>) {
    for message in collect_messages(task) {
        let next = app.update(message);
        run_task(app, next);
    }
}

/// Hands the completions in `messages` straight to the restore handler, so
/// the git commit the restore queues stays queued (`update` would drain it).
pub(super) fn finish_restore(app: &mut Signex, messages: Vec<Message>) {
    for message in messages {
        let Message::File(FileMsg::HistoryRestoreFinished {
            request,
            sha,
            edit_stamp,
            result,
        }) = message
        else {
            panic!("expected a restore completion, got {message:?}");
        };
        let _ = app.handle_history_restore_finished(request, &sha, edit_stamp, result);
    }
}

/// Clicks Restore on `sha` and runs the worker it starts to completion.
pub(super) fn restore(app: &mut Signex, sha: &str) {
    let task = app.handle_history_restore_clicked(sha);
    finish_restore(app, collect_messages(task));
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
    restore(&mut app, &sha);
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
    restore(&mut app, &sha);
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

    restore(&mut app, &sha);
    assert!(
        app.document_state
            .is_document_dirty(&app.document_state.tabs[0].path),
        "load repair leaves it dirty"
    );
    assert!(app.document_state.is_document_dirty(&path));
    assert_eq!(app.document_state.pending_git_commits.len(), 1);
    let queued = app.document_state.pending_git_commits.remove(0);
    assert_eq!(queued.message, "Restore part.snxfpt");
    // Running now, as the dispatcher would have it: its repo is busy.
    app.document_state
        .active_git_repos
        .insert(queued.project_root.clone());

    // The restore commit is now running (key still in flight): it reads the
    // restored bytes before the save below lands.
    let adapter = LocalGitProjectAdapter::open_or_init(dir.path().to_path_buf()).expect("repo");
    adapter
        .commit_path(&queued.rel_path, &queued.message)
        .expect("commit");
    let save = app.update(Message::File(FileMsg::Save));
    run_task(&mut app, save);
    assert_eq!(
        app.document_state.pending_git_commits.len(),
        1,
        "queued behind the running restore commit"
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
    assert!(app.document_state.unbound_git_commits.is_empty());
    // The later save is what HEAD holds: the file as the save wrote it.
    let repo = git2::Repository::open(dir.path()).expect("repo");
    let head = repo.head().expect("head").peel_to_tree().expect("tree");
    let entry = head.get_path(Path::new("part.snxfpt")).expect("entry");
    let blob = repo.find_blob(entry.id()).expect("blob");
    assert_eq!(
        blob.content(),
        std::fs::read(&path).expect("read").as_slice(),
        "HEAD holds the saved bytes"
    );
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
    assert!(!app.document_state.is_document_dirty(path));
    assert!(
        !app.document_state
            .is_document_dirty(&app.document_state.tabs[0].path)
    );
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
        crate::app::FootprintDocument::new(path.clone(), file),
    );
    let before = log_len(dir.path());
    restore(&mut app, &sha);
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
        crate::app::SymbolDocument::new(path.clone(), file),
    );
    let before = log_len(dir.path());
    restore(&mut app, &sha);
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
    restore(&mut app, &sha);
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
    restore(&mut app, &sha);
    assert!(app.document_state.pending_git_commits.is_empty());
    assert!(app.document_state.inflight_git_commits.is_empty());
}

#[test]
fn enqueueing_a_file_twice_while_one_runs_queues_exactly_one_more_with_the_newest_message() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("a.snxsch");
    let mut app = open_tab(dir.path(), &path, TabKind::Schematic);
    app.commit_save_to_project_git(&path, "first");
    // Running now: out of the queue, key still in flight, its repo busy.
    let running = app.document_state.pending_git_commits.remove(0);
    app.document_state
        .active_git_repos
        .insert(running.project_root.clone());
    app.commit_save_to_project_git(&path, "second");
    app.commit_save_to_project_git(&path, "third");
    // Invariant: saves that land while a commit of the file runs coalesce
    // into ONE commit queued behind it, with the newest message.
    let queued = &app.document_state.pending_git_commits;
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].message, "third");
    let key = (running.project_root.clone(), running.rel_path.clone());
    let _ = app.handle_project_git_commit_done(
        running.project_root.clone(),
        running.rel_path.clone(),
        Ok(None),
    );
    // The released commit was drained into a task and is the one now in flight.
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
fn a_commit_queued_behind_a_running_one_carries_the_newest_message_and_runs_after_an_err() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("a.snxsch");
    let mut app = open_tab(dir.path(), &path, TabKind::Schematic);
    app.commit_save_to_project_git(&path, "first");
    let running = app.document_state.pending_git_commits.remove(0);
    app.commit_save_to_project_git(&path, "second");
    app.commit_save_to_project_git(&path, "third");
    // The running commit failed: the one queued behind it still has to run.
    let _ = app.handle_project_git_commit_done(
        running.project_root.clone(),
        running.rel_path.clone(),
        Err(crate::app::GitCommitFailure::Failed("boom".into())),
    );
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
}

// ---- #859: the restore runs on a worker; only the request still tracked lands.

pub(super) fn click_restore(app: &mut Signex, sha: &str) -> Vec<Message> {
    let task = app.handle_history_restore_clicked(sha);
    collect_messages(task)
}

/// A footprint whose description is `stem`, so two fixtures never share a
/// commit sha or a log line (the diagnostics ring is global to the binary).
fn footprint_text_for(name: &str, stem: &str) -> String {
    use signex_library::{Footprint, FootprintFile};
    let mut footprint = Footprint::empty(name);
    footprint.description = stem.to_owned();
    FootprintFile::from_footprint(footprint)
        .to_toml_string()
        .expect("encode")
}

pub(super) fn named_footprint_fixture(stem: &str) -> (tempfile::TempDir, PathBuf, String, Signex) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join(format!("{stem}.snxfpt"));
    let sha = commit_revision(dir.path(), &path, &footprint_text_for("old", stem));
    commit_revision(dir.path(), &path, &footprint_text_for("new", stem));
    let app = open_tab(dir.path(), &path, TabKind::FootprintEditor(path.clone()));
    (dir, path, sha, app)
}

pub(super) fn footprint_fixture() -> (tempfile::TempDir, PathBuf, String, Signex) {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    named_footprint_fixture(&format!("part{n}"))
}

#[test]
fn clicking_restore_changes_nothing_until_the_worker_completes() {
    let (_dir, path, sha, mut app) = footprint_fixture();
    let before = std::fs::read_to_string(&path).expect("read");
    let task = app.handle_history_restore_clicked(&sha);
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        before,
        "update() must not touch the working tree"
    );
    assert!(app.document_state.pending_restores.contains(&path));
    assert!(app.document_state.pending_git_commits.is_empty());
    let messages = collect_messages(task);
    assert_eq!(messages.len(), 1);
    finish_restore(&mut app, messages);
    let after = std::fs::read_to_string(&path).expect("read");
    assert!(
        after.contains("name = \"old\""),
        "restored the old revision"
    );
    assert!(!app.document_state.pending_restores.contains(&path));
    assert_eq!(app.document_state.pending_git_commits.len(), 1);
}

#[test]
fn a_second_restore_click_while_one_runs_is_ignored() {
    let (_dir, _path, sha, mut app) = footprint_fixture();
    let first = app.handle_history_restore_clicked(&sha);
    let second = app.handle_history_restore_clicked(&sha);
    assert_eq!(first.units(), 1);
    assert_eq!(second.units(), 0, "no second worker");
    finish_restore(&mut app, collect_messages(first));
    assert_eq!(
        app.document_state.pending_git_commits.len(),
        1,
        "one commit"
    );
}

#[test]
fn a_restore_whose_tab_closed_meanwhile_installs_and_commits_nothing() {
    let (_dir, path, sha, mut app) = footprint_fixture();
    let messages = click_restore(&mut app, &sha);
    let _ = app.close_tab_now(0);
    finish_restore(&mut app, messages);
    assert!(!app.document_state.footprint_editors.contains_key(&path));
    assert!(!app.document_state.is_document_dirty(&path));
    assert!(app.document_state.pending_git_commits.is_empty());
    assert!(!app.document_state.pending_restores.contains(&path));
}

#[test]
fn closing_all_documents_cancels_a_running_restore() {
    let (_dir, path, sha, mut app) = footprint_fixture();
    let messages = click_restore(&mut app, &sha);
    let _ = app.update(Message::ContextMenu(crate::app::ContextMenuMsg::TabAction(
        crate::app::TabContextAction::CloseAll,
    )));
    finish_restore(&mut app, messages);
    assert!(!app.document_state.footprint_editors.contains_key(&path));
    assert!(app.document_state.pending_git_commits.is_empty());
}

#[test]
fn a_restore_finishing_after_a_tab_switch_restores_its_tab_without_taking_focus() {
    let (dir, path, sha, mut app) = footprint_fixture();
    let other = dir.path().join("other.snxsym");
    app.document_state.tabs.push(TabInfo {
        title: "other".into(),
        path: other.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(other),
    });
    let messages = click_restore(&mut app, &sha);
    app.document_state.active_tab = 1;
    finish_restore(&mut app, messages);
    assert_eq!(app.document_state.active_tab, 1, "the user's tab stays");
    assert!(
        app.document_state.footprint_editors.contains_key(&path),
        "the restored tab still follows its file"
    );
    assert_eq!(
        app.document_state.tabs[1].path.file_name().unwrap(),
        "other.snxsym"
    );
}

#[test]
fn a_restore_that_fails_in_git_reports_the_card_and_keeps_the_tab() {
    let (_dir, path, _sha, mut app) = footprint_fixture();
    let messages = click_restore(&mut app, "0000000000000000000000000000000000000000");
    finish_restore(&mut app, messages);
    let card = format!(
        "{:?}",
        app.document_state.error_notice.as_ref().expect("card")
    );
    assert!(
        card.contains(&format!("Could not restore {} to 0000", file_name(&path))),
        "{card}"
    );
    assert!(!app.document_state.pending_restores.contains(&path));
    assert!(app.document_state.pending_git_commits.is_empty());
    assert_eq!(app.document_state.tabs.len(), 1);
}

// ---- #859: nothing may write or replace a document while its restore runs.

pub(super) fn file_name(path: &Path) -> String {
    path.file_name()
        .expect("name")
        .to_string_lossy()
        .into_owned()
}

pub(super) fn logged(needle: &str) -> bool {
    crate::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains(needle))
}

pub(super) fn marker_editor(app: &mut Signex, path: &Path, name: &str) {
    let file =
        signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty(name));
    app.document_state.footprint_editors.insert(
        path.to_path_buf(),
        crate::app::FootprintDocument::new(path.to_path_buf(), file),
    );
}

pub(super) fn editor_name(app: &Signex, path: &Path) -> String {
    app.document_state.footprint_editors[path]
        .primitive()
        .name
        .clone()
}

#[test]
fn saving_while_a_restore_runs_is_refused_and_the_restored_bytes_are_what_gets_committed() {
    let _ = crate::diagnostics::init_logging();
    let (_dir, path, sha, mut app) = footprint_fixture();
    marker_editor(&mut app, &path, "mine");
    let messages = click_restore(&mut app, &sha);
    let before = std::fs::read_to_string(&path).expect("read");

    let save = app.update(Message::File(FileMsg::Save));
    run_task(&mut app, save);
    assert!(logged(&format!(
        "A History restore of {} is still running, so the save was refused",
        file_name(&path)
    )));
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        before,
        "no write"
    );

    finish_restore(&mut app, messages);
    let restored = std::fs::read_to_string(&path).expect("read");
    assert!(restored.contains("name = \"old\""));
    assert_eq!(app.document_state.pending_git_commits.len(), 1);
    assert!(
        app.document_state.pending_git_commits[0]
            .message
            .starts_with("Restore ")
    );
    // Saving works again once the restore is done.
    assert!(app.refuse_save_while_restoring(&path).is_ok());
}

#[test]
fn save_as_and_save_all_paths_are_refused_for_the_restoring_document_only() {
    let (dir, path, sha, mut app) = footprint_fixture();
    let _ = click_restore(&mut app, &sha);
    let err = app.refuse_save_while_restoring(&path).unwrap_err();
    assert!(
        err.to_string().starts_with(&format!(
            "A History restore of {} is still running",
            file_name(&path)
        )),
        "{err}"
    );
    assert!(
        app.refuse_save_while_restoring(&dir.path().join("other.snxsch"))
            .is_ok()
    );
    assert!(app.save_active_document().is_err());
    assert!(
        app.save_active_document_as(&path, dir.path().join("x.snxfpt"))
            .is_err()
    );
    assert!(app.save_primitive_tab_at(&path).is_err());
}

pub(super) fn edit_description(app: &mut Signex, text: &str) {
    use crate::dock::DockMessage;
    use crate::panels::PanelMsg;
    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::FpEditorSetFootprintDescription(text.into()),
    )));
}

fn description(app: &Signex, path: &Path) -> String {
    app.document_state.footprint_editors[path]
        .primitive()
        .description
        .clone()
}

fn restore_card(app: &Signex) -> String {
    format!(
        "{:?}",
        app.document_state.error_notice.as_ref().expect("card")
    )
}

#[test]
fn a_panel_edit_made_while_a_restore_runs_is_kept_not_replaced() {
    let (_dir, path, sha, mut app) = footprint_fixture();
    marker_editor(&mut app, &path, "mine");
    let messages = click_restore(&mut app, &sha);
    // A real edit on the clean tab.
    edit_description(&mut app, "edited");
    finish_restore(&mut app, messages);
    assert_eq!(description(&app, &path), "edited", "edit not replaced");
    assert_eq!(editor_name(&app, &path), "mine");
    assert!(app.document_state.pending_git_commits.is_empty());
    assert!(restore_card(&app).contains("was edited while its restore"));
}

#[test]
fn an_edit_after_the_confirmed_discard_click_is_kept_too() {
    let (_dir, path, sha, mut app) = footprint_fixture();
    marker_editor(&mut app, &path, "mine");
    edit_description(&mut app, "first");
    // Dirty draft: the first click arms the confirm, the second restores.
    assert!(click_restore(&mut app, &sha).is_empty());
    let messages = click_restore(&mut app, &sha);
    assert_eq!(messages.len(), 1);
    edit_description(&mut app, "second");
    finish_restore(&mut app, messages);
    assert_eq!(description(&app, &path), "second");
    assert!(app.document_state.pending_git_commits.is_empty());
    assert!(restore_card(&app).contains("was edited while its restore"));
}

#[test]
fn a_net_colour_change_on_a_dirty_schematic_during_a_restore_is_kept() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("a.snxsch");
    let text = signex_types::format::SnxSchematic::new(Default::default())
        .write_string()
        .expect("encode");
    let sha = commit_revision(dir.path(), &path, &text);
    commit_revision(dir.path(), &path, &format!("{text}\n"));
    let mut app = open_tab(dir.path(), &path, TabKind::Schematic);
    let engine = signex_engine::Engine::new(Default::default()).expect("engine");
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.active_path = Some(path.clone());
    app.document_state.mark_document_dirty(&path);
    {
        let path = app.document_state.tabs[0].path.clone();
        app.document_state.mark_document_dirty(&path);
    }

    assert!(click_restore(&mut app, &sha).is_empty(), "arms the confirm");
    let messages = click_restore(&mut app, &sha);
    assert_eq!(messages.len(), 1);
    let wire = uuid::Uuid::new_v4();
    app.record_net_color_undo();
    app.ui_state
        .wire_color_overrides
        .insert(wire, signex_types::theme::Color::new(9, 0, 0, 255));
    app.mark_net_colors_dirty(&path);
    finish_restore(&mut app, messages);
    assert!(app.ui_state.wire_color_overrides.contains_key(&wire));
    assert!(app.document_state.engines.contains_key(&path));
    assert!(app.document_state.pending_git_commits.is_empty());
    assert!(restore_card(&app).contains("was edited while its restore"));
}

#[test]
fn closing_a_dirty_tab_during_a_restore_parks_the_draft_and_warns() {
    let _ = crate::diagnostics::init_logging();
    let (_dir, path, sha, mut app) = footprint_fixture();
    marker_editor(&mut app, &path, "mine");
    edit_description(&mut app, "draft");
    assert!(click_restore(&mut app, &sha).is_empty());
    let messages = click_restore(&mut app, &sha);
    let _ = app.close_tab_parking_dirty(0);
    assert!(app.document_state.tabs.is_empty());
    finish_restore(&mut app, messages);
    assert_eq!(
        description(&app, &path),
        "draft",
        "draft parked, not replaced"
    );
    assert!(app.document_state.footprint_editors[&path].is_dirty());
    assert!(logged(&format!(
        "{} has unsaved edits but its file is now at {sha}; saving overwrites that restore",
        file_name(&path)
    )));
    assert!(!logged(&format!(
        "{} was restored to {sha} on disk, but its tab or project was closed",
        path.display()
    )));
}

#[test]
fn a_restore_does_not_land_in_a_tab_reopened_while_it_ran() {
    let _ = crate::diagnostics::init_logging();
    let (_dir, path, sha, mut app) = footprint_fixture();
    let messages = click_restore(&mut app, &sha);
    let _ = app.close_tab_now(0);
    // The same file is reopened and edited before the old restore completes.
    marker_editor(&mut app, &path, "reopened");
    app.document_state.tabs.push(TabInfo {
        title: "part".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.mark_document_dirty(&path);

    finish_restore(&mut app, messages);
    assert_eq!(editor_name(&app, &path), "reopened");
    assert!(app.document_state.pending_git_commits.is_empty());
    assert!(logged(&format!(
        "{} has unsaved edits but its file is now at {sha}",
        file_name(&path)
    )));
}

#[test]
fn a_cancelled_restore_that_rewrote_the_file_says_so() {
    let _ = crate::diagnostics::init_logging();
    let (_dir, path, sha, mut app) = footprint_fixture();
    let messages = click_restore(&mut app, &sha);
    app.document_state.tabs.clear();
    app.cancel_in_flight_reads();
    finish_restore(&mut app, messages);
    assert!(
        std::fs::read_to_string(&path)
            .expect("read")
            .contains("name = \"old\"")
    );
    assert!(logged(&format!(
        "{} was restored to {sha} on disk",
        path.display()
    )));
}

#[test]
fn save_all_reports_the_restoring_file_and_still_saves_the_others() {
    let (dir, path, sha, mut app) = footprint_fixture();
    let _ = click_restore(&mut app, &sha);
    let err = app.try_save_dirty_path(&path).unwrap_err();
    assert!(err.to_string().starts_with(&format!(
        "A History restore of {} is still running",
        file_name(&path)
    )));
    // Another file is not held up by it.
    let other = dir.path().join("other.snxsym");
    app.document_state.symbol_editors.insert(
        other.clone(),
        crate::app::SymbolDocument::new(
            other.clone(),
            signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("o")),
        ),
    );
    app.try_save_dirty_path(&other).expect("other file saves");
    assert!(other.exists());
}

/// A restored `snxsch/1.7` blob is read-only: it matches disk and can never
/// be saved, so the tab must not stay dirty and quit must not ask (#1402).
#[test]
fn restoring_a_newer_minor_schematic_leaves_it_clean_and_quit_does_not_prompt() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("a.snxsch");
    let current = signex_types::format::SnxSchematic::new(Default::default())
        .write_string()
        .expect("encode");
    let newer = current.replacen("snxsch/1", "snxsch/1.7", 1);
    assert!(newer.contains("snxsch/1.7"), "{newer}");
    let sha = commit_revision(dir.path(), &path, &newer);
    commit_revision(dir.path(), &path, &current);
    let mut app = open_tab(dir.path(), &path, TabKind::Schematic);
    let mut engine =
        signex_engine::Engine::new(signex_types::schematic::SchematicSheet::default()).unwrap();
    engine.set_path(Some(path.clone()));
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.active_path = Some(path.clone());

    let messages = click_restore(&mut app, &sha);
    finish_restore(&mut app, messages);

    let engine = &app.document_state.engines[&path];
    assert!(engine.document().is_read_only(), "the restored sheet loads");
    assert!(
        !app.document_state
            .is_document_dirty(&app.document_state.tabs[0].path)
    );
    assert!(!app.document_state.is_document_dirty(&path));
    let _task = app.handle_app_quit_requested();
    assert!(app.ui_state.app_quit_confirm.is_none(), "quit must not ask");
}
