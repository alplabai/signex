//! #833 — commits to one repository run strictly one after another.

use std::path::PathBuf;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, ProjectMsg, Signex};

fn git_app(dir: &std::path::Path) -> Signex {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.document_state.projects[0].data.enable_git = true;
    app
}

#[test]
fn two_rapid_saves_to_one_repo_commit_in_order_one_at_a_time() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = git_app(dir.path());
    let first = dir.path().join("a.snxsch");
    let second = dir.path().join("b.snxsch");

    app.commit_save_to_project_git(&first, "Save a");
    app.commit_save_to_project_git(&second, "Save b");
    let _first_task = app.drain_pending_git_commits();

    assert_eq!(
        app.document_state.pending_git_commits.len(),
        1,
        "only one commit may be in flight for a repo; the second must wait"
    );
    assert_eq!(
        app.document_state.pending_git_commits[0].message, "Save b",
        "the waiting commit is the later save"
    );

    // The first commit completes: the second is released, and only then.
    let _ = app.update(Message::Project(ProjectMsg::GitCommitDone {
        project_root: dir.path().to_path_buf(),
        rel_path: PathBuf::from("a.snxsch"),
        result: Ok(Some("deadbeef".to_string())),
    }));
    assert!(
        app.document_state.pending_git_commits.is_empty(),
        "the second commit is released by the update that handled the first's completion"
    );
}

/// #1205 — a save in a shared folder with no `.git` is refused by the guard;
/// the user hears about it once per project, not on every save.
#[test]
fn a_paused_repository_is_reported_once_per_project_not_every_save() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("A.snxprj"), b"").unwrap();
    std::fs::write(dir.path().join("B.snxprj"), b"").unwrap();
    let mut app = git_app(dir.path());
    let file = dir.path().join("a.snxsch");

    app.commit_save_to_project_git(&file, "Save 1");
    let task = app.drain_pending_git_commits();
    crate::app::handlers::menu::export::settle(&mut app, task);
    let notice = app.document_state.error_notice.take().expect("reported");
    assert!(notice.detail.contains("paused"), "{notice:?}");
    assert!(!dir.path().join(".git").exists(), "no repository was made");

    app.commit_save_to_project_git(&file, "Save 2");
    let task = app.drain_pending_git_commits();
    crate::app::handlers::menu::export::settle(&mut app, task);
    assert!(
        app.document_state.error_notice.is_none(),
        "the second refusal must stay quiet"
    );
}

fn refused(app: &mut Signex, root: &std::path::Path) {
    let _ = app.handle_project_git_commit_done(
        root.to_path_buf(),
        PathBuf::from("a.snxsch"),
        Err(crate::app::GitCommitFailure::Refused("shared".into())),
    );
}

/// Refusal lines for one project folder. The diagnostics ring is shared by
/// every test in the process and tests run in parallel, so a bare message
/// prefix also counts refusals other tests log at the same time; each test's
/// temp folder is unique, so the folder path scopes the count.
fn refusal_lines(root: &std::path::Path) -> usize {
    let prefix = format!("[git] a.snxsch in {} was not committed", root.display());
    crate::diagnostics::recent_entries()
        .iter()
        .filter(|e| e.message.starts_with(&prefix))
        .count()
}

/// Every refused auto-commit reaches Messages; only the card is once.
#[test]
fn every_refused_commit_is_logged_but_the_card_is_raised_once() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = git_app(dir.path());
    let before = refusal_lines(dir.path());

    refused(&mut app, dir.path());
    assert!(app.document_state.error_notice.take().is_some());
    refused(&mut app, dir.path());
    assert!(app.document_state.error_notice.is_none());

    assert_eq!(
        refusal_lines(dir.path()) - before,
        2,
        "one Messages line per refusal"
    );
}

/// Regression for the Windows-only flake: refusals logged for ANOTHER
/// project folder, in the same process, must not move this folder's count.
#[test]
fn refusal_lines_ignore_refusals_logged_for_other_folders() {
    let _ = crate::diagnostics::init_logging();
    let mine = tempfile::tempdir().expect("tempdir");
    let other = tempfile::tempdir().expect("tempdir");
    let mut app = git_app(mine.path());
    let mut other_app = git_app(other.path());
    let before = refusal_lines(mine.path());

    refused(&mut other_app, other.path());
    refused(&mut other_app, other.path());
    refused(&mut app, mine.path());

    assert_eq!(refusal_lines(mine.path()) - before, 1);
}

/// A commit that works lifts the pause, so a later refusal is news again.
#[test]
fn a_successful_commit_lifts_the_pause() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = git_app(dir.path());
    refused(&mut app, dir.path());
    assert!(app.document_state.paused_git_projects.contains(dir.path()));
    app.document_state.error_notice = None;

    let _ = app.handle_project_git_commit_done(
        dir.path().to_path_buf(),
        PathBuf::from("a.snxsch"),
        Ok(None),
    );
    assert!(app.document_state.paused_git_projects.is_empty());

    refused(&mut app, dir.path());
    assert!(app.document_state.error_notice.is_some());
}

/// Only a guard refusal is "paused"; a git failure is an ordinary warning.
#[test]
fn a_git_failure_is_not_reported_as_paused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = git_app(dir.path());
    let _ = app.handle_project_git_commit_done(
        dir.path().to_path_buf(),
        PathBuf::from("a.snxsch"),
        Err(crate::app::GitCommitFailure::Failed("boom".into())),
    );
    assert!(app.document_state.error_notice.is_none());
    assert!(app.document_state.paused_git_projects.is_empty());
}

/// #858 / #893: a document commit that cannot take the repository commit lock
/// (another process holds it past the wait) is a failed commit, not a failed
/// save: the file is written and the Messages panel says the commit failed
/// and why.
#[test]
fn a_commit_that_cannot_take_the_repository_lock_is_reported_not_committed() {
    let _ = crate::diagnostics::init_logging();
    signex_library::git_identity::test_identity::install();
    let dir = tempfile::tempdir().expect("tempdir");
    git2::Repository::init(dir.path()).expect("init");
    let file = dir.path().join("a.snxsch");
    std::fs::write(&file, b"saved").unwrap();
    let mut app = git_app(dir.path());

    let lock_path = dir.path().join(".git").join("signex").join("commit.lock");
    std::fs::create_dir_all(lock_path.parent().unwrap()).unwrap();
    let held = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .unwrap();
    held.lock().unwrap();

    app.commit_save_to_project_git(&file, "Save a");
    let task = app.drain_pending_git_commits();
    crate::app::handlers::menu::export::settle(&mut app, task);
    drop(held);

    assert_eq!(std::fs::read(&file).unwrap(), b"saved", "the save stands");
    let reported = crate::diagnostics::recent_entries().iter().any(|e| {
        e.message.contains("commit_path(a.snxsch) failed")
            && e.message.contains("another Signex process is committing")
    });
    assert!(reported, "the failed commit reaches Messages");
}
