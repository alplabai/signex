//! A project git commit records the bytes the save wrote or the restore
//! restored, not whatever the file holds when the commit worker gets to it.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::restore_commit_tests::{
    collect_messages, commit_revision, file_name, finish_restore, footprint_fixture, open_tab,
    run_task,
};
use crate::app::{Signex, TabInfo, TabKind};
use crate::persist::{DocumentSaveDone, SaveOutcome};

fn head_blob(dir: &Path, rel: &str) -> Vec<u8> {
    let repo = git2::Repository::open(dir).expect("repo");
    let tree = repo
        .head()
        .expect("head")
        .peel_to_commit()
        .expect("commit")
        .tree()
        .expect("tree");
    let entry = tree.get_path(Path::new(rel)).expect("entry");
    repo.find_blob(entry.id()).expect("blob").content().to_vec()
}

fn head_subject(dir: &Path) -> String {
    let repo = git2::Repository::open(dir).expect("repo");
    let head = repo.head().expect("head").peel_to_commit().expect("commit");
    head.message()
        .expect("utf-8")
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// A project with one committed sheet, open with an engine behind its tab.
fn sheet_fixture(stem: &str) -> (tempfile::TempDir, PathBuf, Signex) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join(format!("{stem}.snxsch"));
    commit_revision(dir.path(), &path, "base revision");
    let mut app = open_tab(dir.path(), &path, TabKind::Schematic);
    app.document_state.tabs.clear();
    let engine = signex_engine::Engine::new(signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        ..Default::default()
    })
    .expect("engine");
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: file_name(&path),
        path: path.clone(),
        cached_document: None,
        dirty: true,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.clone());
    app.document_state.dirty_paths.insert(path.clone());
    (dir, path, app)
}

/// Runs a save of `bytes` to completion and handles its completion message,
/// which queues the "Save X" commit without running it.
fn save_without_running_the_commit(app: &mut Signex, path: &Path, bytes: &[u8]) {
    let revision = app.document_state.engines[path].revision();
    let job = app
        .document_state
        .document_saves
        .begin(path.to_path_buf(), bytes.to_vec());
    let saved = job.bytes();
    assert_eq!(job.run(), Ok(SaveOutcome::Written));
    let _ = app.handle_document_saved(DocumentSaveDone {
        path: path.to_path_buf(),
        revision,
        net_color_revision: 0,
        save_as_from: None,
        bytes: saved,
        result: Ok(SaveOutcome::Written),
    });
}

#[test]
fn an_external_edit_between_the_save_and_its_commit_does_not_change_the_committed_blob() {
    let (dir, path, mut app) = sheet_fixture("ext-edit");
    save_without_running_the_commit(&mut app, &path, b"the saved bytes");
    assert_eq!(app.document_state.pending_git_commits.len(), 1);

    // Another program rewrites the file before the commit worker starts.
    std::fs::write(&path, b"someone else's edit").expect("external edit");
    let commits = app.drain_pending_git_commits();
    run_task(&mut app, commits);

    let rel = file_name(&path);
    assert_eq!(head_subject(dir.path()), format!("Save {rel}"));
    assert_eq!(head_blob(dir.path(), &rel), b"the saved bytes");
    assert_eq!(
        std::fs::read(&path).expect("read"),
        b"someone else's edit",
        "the commit never touches the working file"
    );
}

#[test]
fn a_save_queued_behind_a_running_commit_keeps_its_own_bytes_and_the_newest_wins() {
    let (dir, path, mut app) = sheet_fixture("behind-running");
    save_without_running_the_commit(&mut app, &path, b"first");
    // The first commit is running now: out of the queue, its key in flight.
    let running = app.document_state.pending_git_commits.remove(0);
    app.document_state
        .active_git_repos
        .insert(running.project_root.clone());

    app.commit_bytes_to_project_git(&path, "Save second", Arc::from(&b"second"[..]));
    app.commit_bytes_to_project_git(&path, "Save third", Arc::from(&b"third"[..]));

    let queued = &app.document_state.pending_git_commits;
    assert_eq!(queued.len(), 1, "one commit queued behind the running one");
    assert_eq!(queued[0].message, "Save third");
    assert_eq!(queued[0].content.as_deref(), Some(&b"third"[..]));

    let done = app.handle_project_git_commit_done(
        running.project_root.clone(),
        running.rel_path.clone(),
        Ok(None),
    );
    run_task(&mut app, done);
    let rel = file_name(&path);
    assert_eq!(head_blob(dir.path(), &rel), b"third");
}

#[test]
fn the_restore_commit_records_the_restored_revision_even_if_the_file_is_edited_again() {
    let (dir, path, sha, mut app) = footprint_fixture();
    let rel = file_name(&path);
    let restored = {
        let repo = git2::Repository::open(dir.path()).expect("repo");
        let commit = repo
            .find_commit(git2::Oid::from_str(&sha).expect("sha"))
            .expect("commit");
        let entry = commit
            .tree()
            .expect("tree")
            .get_path(Path::new(&rel))
            .expect("entry");
        repo.find_blob(entry.id()).expect("blob").content().to_vec()
    };

    let task = app.handle_history_restore_clicked(&sha);
    finish_restore(&mut app, collect_messages(task));
    assert_eq!(app.document_state.pending_git_commits.len(), 1);

    // An external edit lands between the restore and its commit.
    std::fs::write(&path, b"edited after the restore").expect("external edit");
    let commits = app.drain_pending_git_commits();
    run_task(&mut app, commits);

    assert_eq!(head_subject(dir.path()), format!("Restore {rel}"));
    assert_eq!(head_blob(dir.path(), &rel), restored);
}

#[test]
fn a_message_only_enqueue_still_commits_the_file_as_it_is() {
    let (dir, path, mut app) = sheet_fixture("unbound");
    std::fs::write(&path, b"on disk now").expect("write");
    app.commit_save_to_project_git(&path, "Save unbound.snxsch");

    let commits = app.drain_pending_git_commits();
    run_task(&mut app, commits);

    assert_eq!(head_blob(dir.path(), &file_name(&path)), b"on disk now");
}

#[test]
fn a_restore_is_not_held_back_by_a_save_commit_that_carries_its_bytes() {
    let (_dir, path, sha, mut app) = footprint_fixture();
    app.commit_bytes_to_project_git(&path, "Save part", Arc::from(&b"saved"[..]));
    assert_eq!(app.document_state.pending_git_commits.len(), 1);
    assert!(app.document_state.unbound_git_commits.is_empty());

    let task = app.handle_history_restore_clicked(&sha);

    assert!(
        app.document_state.pending_restores.contains(&path),
        "the restore started"
    );
    assert!(app.document_state.error_notice.is_none());
    drop(task);
}
