//! #1090 — a History restore must not run while a save's project-git commit
//! is still queued or running: the commit worker reads the file when it
//! starts, so a restore in between would be committed under the save's label.

use std::path::{Path, PathBuf};

use signex_library::adapters::local_git_project::LocalGitProjectAdapter;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Signex, TabInfo, TabKind};

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

fn open_tab(dir: &Path, path: &Path) -> Signex {
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    app.document_state.projects[0].data.enable_git = true;
    app.document_state.tabs.push(TabInfo {
        title: "part".into(),
        path: path.to_path_buf(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::FootprintEditor(path.to_path_buf()),
    });
    app.document_state.active_tab = 0;
    app
}

fn footprint_text(name: &str) -> String {
    use signex_library::{Footprint, FootprintFile};
    FootprintFile::from_footprint(Footprint::empty(name))
        .to_toml_string()
        .expect("encode")
}

#[test]
fn restore_is_refused_while_a_save_commit_is_queued() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path: PathBuf = dir.path().join("part.snxfpt");
    let old = footprint_text("old");
    let sha = commit_revision(dir.path(), &path, &old);
    let new = footprint_text("new");
    std::fs::write(&path, &new).expect("save newer content");
    let mut app = open_tab(dir.path(), &path);
    // The save queued its commit; the worker has not started yet.
    app.commit_save_to_project_git(&path, "Save part.snxfpt");

    app.handle_history_restore_clicked(&sha);

    assert_eq!(std::fs::read_to_string(&path).expect("read"), new);
    let pending = &app.document_state.pending_git_commits;
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].message, "Save part.snxfpt");
    assert!(app.document_state.error_notice.is_some(), "user is told");
}

#[test]
fn restore_runs_when_no_commit_is_pending() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path: PathBuf = dir.path().join("part.snxfpt");
    let old = footprint_text("old");
    let sha = commit_revision(dir.path(), &path, &old);
    std::fs::write(&path, footprint_text("new")).expect("save");
    let mut app = open_tab(dir.path(), &path);

    app.handle_history_restore_clicked(&sha);

    assert_eq!(std::fs::read_to_string(&path).expect("read"), old);
    let pending = &app.document_state.pending_git_commits;
    assert_eq!(pending.len(), 1);
    assert!(pending[0].message.starts_with("Restore "));
}
