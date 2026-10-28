//! A project git commit records the bytes the save wrote or the restore
//! restored, not whatever the file holds when the commit worker gets to it.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::restore_commit_tests::{
    collect_messages, commit_revision, file_name, finish_restore, footprint_fixture, open_tab,
    run_task,
};
use crate::app::state::CommitBytes;
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

fn commit_bytes(bytes: &[u8], sequence: u64) -> CommitBytes {
    CommitBytes {
        bytes: Arc::new(bytes.to_vec()),
        sequence,
    }
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
    let sequence = job.sequence();
    assert_eq!(job.run(), Ok(SaveOutcome::Written));
    let _ = app.handle_document_saved(DocumentSaveDone {
        path: path.to_path_buf(),
        revision,
        net_color_revision: 0,
        save_as_from: None,
        bytes: saved,
        sequence,
        recovery_hold: None,
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

    app.commit_bytes_to_project_git(&path, "Save second", commit_bytes(b"second", 2));
    app.commit_bytes_to_project_git(&path, "Save third", commit_bytes(b"third", 3));

    let queued = &app.document_state.pending_git_commits;
    assert_eq!(queued.len(), 1, "one commit queued behind the running one");
    assert_eq!(queued[0].message, "Save third");
    assert_eq!(
        queued[0].content.as_ref().map(|c| c.as_slice()),
        Some(&b"third"[..])
    );

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
    app.commit_bytes_to_project_git(&path, "Save part", commit_bytes(b"saved", 1));
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

/// `(subject, blob)` of every commit of `rel`, newest first.
fn log_of(dir: &Path, rel: &str) -> Vec<(String, Vec<u8>)> {
    let repo = git2::Repository::open(dir).expect("repo");
    let mut walk = repo.revwalk().expect("walk");
    walk.push_head().expect("head");
    walk.map(|oid| {
        let commit = repo.find_commit(oid.expect("oid")).expect("commit");
        let entry = commit
            .tree()
            .expect("tree")
            .get_path(Path::new(rel))
            .expect("entry");
        let blob = repo.find_blob(entry.id()).expect("blob").content().to_vec();
        let subject = commit.message().unwrap_or_default().lines().next();
        (subject.unwrap_or_default().to_owned(), blob)
    })
    .collect()
}

#[test]
fn a_restore_queues_behind_a_pending_save_commit_instead_of_replacing_it() {
    let (dir, path, sha, mut app) = footprint_fixture();
    let rel = file_name(&path);
    // Save All: this file's "Save" commit waits in the queue (bytes S).
    app.commit_bytes_to_project_git(&path, "Save part", commit_bytes(b"S, the last save", 1));
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
    assert_eq!(
        app.document_state.pending_git_commits.len(),
        2,
        "the restore is a second entry, not a merge into the save"
    );
    assert!(app.document_state.inflight_git_commits.len() == 1);
    assert!(app.document_state.unbound_git_commits.is_empty());

    let commits = app.drain_pending_git_commits();
    run_task(&mut app, commits);

    let log = log_of(dir.path(), &rel);
    assert_eq!(log[0], (format!("Restore {rel}"), restored));
    assert_eq!(
        log[1],
        ("Save part".to_owned(), b"S, the last save".to_vec())
    );
    assert!(app.document_state.pending_git_commits.is_empty());
    assert!(app.document_state.inflight_git_commits.is_empty());
    assert!(app.document_state.active_git_repos.is_empty());
}

#[test]
fn a_later_save_coalesces_into_the_newest_queued_entry_of_the_file() {
    let (dir, path, sha, mut app) = footprint_fixture();
    let rel = file_name(&path);
    app.commit_bytes_to_project_git(&path, "Save part", commit_bytes(b"S", 1));
    let task = app.handle_history_restore_clicked(&sha);
    finish_restore(&mut app, collect_messages(task));
    app.commit_bytes_to_project_git(&path, "Save part again", commit_bytes(b"S2", 2));
    app.commit_bytes_to_project_git(&path, "Save part third", commit_bytes(b"S3", 3));

    let queued = &app.document_state.pending_git_commits;
    assert_eq!(
        queued.len(),
        2,
        "the Save stays; later saves merge behind it"
    );
    assert_eq!(queued[0].message, "Save part");

    let commits = app.drain_pending_git_commits();
    run_task(&mut app, commits);
    let log = log_of(dir.path(), &rel);
    assert_eq!(log[0], ("Save part third".to_owned(), b"S3".to_vec()));
    assert_eq!(log[1], ("Save part".to_owned(), b"S".to_vec()));
}

#[test]
fn a_newer_unbound_write_replaces_the_bytes_of_a_pending_restore_commit() {
    let (dir, path, sha, mut app) = footprint_fixture();
    let _ = sha;
    let rel = file_name(&path);
    app.commit_bytes_to_project_git(&path, "Restore part", commit_bytes(b"restored", 0));

    // A primitive-editor save then rewrote the file and asks, without
    // relabelling, for it to be committed.
    std::fs::write(&path, b"newer write").expect("write");
    app.commit_save_to_project_git_if_unqueued(&path, "Save footprint part");

    let queued = &app.document_state.pending_git_commits;
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].message, "Restore part", "the label is kept");
    assert!(
        queued[0].content.is_none(),
        "the newer write is read from disk"
    );
    assert!(app.document_state.unbound_git_commits.len() == 1);
    let commits = app.drain_pending_git_commits();
    run_task(&mut app, commits);
    assert_eq!(head_blob(dir.path(), &rel), b"newer write");
}

#[test]
fn a_commit_of_an_older_save_never_replaces_or_follows_a_newer_one() {
    let (dir, path, mut app) = sheet_fixture("order");
    let rel = file_name(&path);
    app.commit_bytes_to_project_git(&path, "Save new", commit_bytes(b"new", 5));
    app.commit_bytes_to_project_git(&path, "Save old", commit_bytes(b"old", 3));
    let queued = &app.document_state.pending_git_commits;
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].message, "Save new");
    assert_eq!(
        queued[0].content.as_ref().map(|c| c.as_slice()),
        Some(&b"new"[..])
    );

    let commits = app.drain_pending_git_commits();
    run_task(&mut app, commits);
    assert_eq!(head_blob(dir.path(), &rel), b"new");

    // The older completion arrives after the newer one's commit finished.
    app.commit_bytes_to_project_git(&path, "Save older", commit_bytes(b"older", 4));
    assert!(app.document_state.pending_git_commits.is_empty());
    assert!(app.document_state.inflight_git_commits.is_empty());
    assert_eq!(head_blob(dir.path(), &rel), b"new");
}

#[test]
fn a_rename_commit_that_absorbs_a_save_still_counts_as_reading_the_file() {
    let (dir, path, mut app) = sheet_fixture("renamed");
    let old = dir.path().join("before.snxsch");
    app.commit_project_rename_to_git(&old, &path, "Rename before to renamed");

    app.commit_bytes_to_project_git(&path, "Save renamed", commit_bytes(b"saved", 1));

    let queued = &app.document_state.pending_git_commits;
    assert_eq!(queued.len(), 1);
    assert!(queued[0].removed_rel_path.is_some());
    assert!(
        queued[0].reads_working_file(),
        "the rename arm ignores the bytes"
    );
    assert_eq!(app.document_state.unbound_git_commits.len(), 1);
}

#[test]
fn a_restore_waits_for_a_commit_that_reads_the_file_and_runs_after_it_finishes_or_fails() {
    for outcome in [
        Ok(None),
        Err(crate::app::GitCommitFailure::Failed("boom".into())),
    ] {
        let (_dir, path, sha, mut app) = footprint_fixture();
        app.commit_save_to_project_git(&path, "Save part");
        let running = app.document_state.pending_git_commits.remove(0);
        app.document_state
            .active_git_repos
            .insert(running.project_root.clone());

        let refused = app.handle_history_restore_clicked(&sha);
        assert_eq!(refused.units(), 0);
        assert!(!app.document_state.pending_restores.contains(&path));
        assert!(app.document_state.error_notice.take().is_some());

        let _ = app.handle_project_git_commit_done(running.project_root, running.rel_path, outcome);
        assert!(app.document_state.unbound_git_commits.is_empty());
        let task = app.handle_history_restore_clicked(&sha);
        assert!(app.document_state.pending_restores.contains(&path));
        drop(task);
    }
}

#[test]
fn the_restore_guard_finds_the_commit_by_the_projects_case_folding() {
    let (_dir, path, sha, mut app) = footprint_fixture();
    app.document_state.projects[0].case_fold = crate::app::state::scope::CaseFold::case_only();
    // The commit was queued under another spelling of the same file.
    let respelled = path.with_file_name(file_name(&path).to_uppercase());
    app.commit_save_to_project_git(&respelled, "Save part");
    assert_eq!(app.document_state.unbound_git_commits.len(), 1);

    let task = app.handle_history_restore_clicked(&sha);

    assert_eq!(task.units(), 0, "refused");
    assert!(!app.document_state.pending_restores.contains(&path));
}

#[test]
fn the_restore_guard_names_the_same_repo_as_the_commit_queue_for_nested_projects() {
    let (dir, path, sha, mut app) = footprint_fixture();
    // An outer project without version control is listed first and also
    // holds the file; the commit is queued under the inner git project.
    let mut outer = app.document_state.projects[0].clone();
    outer.id = app.document_state.mint_project_id();
    outer.data.dir = dir
        .path()
        .parent()
        .expect("parent")
        .to_string_lossy()
        .into_owned();
    outer.data.enable_git = false;
    app.document_state.projects.insert(0, outer);
    app.commit_save_to_project_git(&path, "Save part");
    assert_eq!(app.document_state.unbound_git_commits.len(), 1);

    let task = app.handle_history_restore_clicked(&sha);

    assert_eq!(task.units(), 0, "refused");
    assert!(!app.document_state.pending_restores.contains(&path));
}

#[test]
fn a_restore_in_a_nested_project_targets_the_inner_git_repo_and_starts_none_outside() {
    let outer_dir = tempfile::tempdir().expect("tempdir");
    let inner = outer_dir.path().join("inner");
    std::fs::create_dir(&inner).expect("inner folder");
    let path = inner.join("part.snxfpt");
    let old = super::restore_commit_tests::footprint_text("old");
    let sha = commit_revision(&inner, &path, &old);
    commit_revision(
        &inner,
        &path,
        &super::restore_commit_tests::footprint_text("new"),
    );
    let mut app = open_tab(&inner, &path, TabKind::FootprintEditor(path.clone()));
    // An outer project without version control also holds the file.
    let mut outer = app.document_state.projects[0].clone();
    outer.id = app.document_state.mint_project_id();
    outer.data.dir = outer_dir.path().to_string_lossy().into_owned();
    outer.data.enable_git = false;
    app.document_state.projects.insert(0, outer);

    let task = app.handle_history_restore_clicked(&sha);
    finish_restore(&mut app, collect_messages(task));

    assert!(
        !outer_dir.path().join(".git").exists(),
        "no repository was started in the outer folder"
    );
    assert!(app.document_state.error_notice.is_none());
    assert_eq!(std::fs::read_to_string(&path).expect("read"), old);
}
