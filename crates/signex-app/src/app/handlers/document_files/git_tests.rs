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
