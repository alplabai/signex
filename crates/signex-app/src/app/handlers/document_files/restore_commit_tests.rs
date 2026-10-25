//! #1025 — a History restore of a footprint or symbol is committed to
//! project git, and the restored bytes stay as they are on disk.

use std::path::{Path, PathBuf};

use signex_library::adapters::local_git_project::LocalGitProjectAdapter;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Signex, TabInfo, TabKind};

/// Commits `text` as `path` in the project repo and returns the sha.
fn commit_revision(dir: &Path, path: &Path, text: &str) -> String {
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

#[test]
fn restoring_a_footprint_queues_one_restore_commit() {
    use signex_library::{Footprint, FootprintFile};
    let dir = tempfile::tempdir().expect("tempdir");
    let path: PathBuf = dir.path().join("part.snxfpt");
    let old = FootprintFile::from_footprint(Footprint::empty("old"))
        .to_toml_string()
        .expect("encode");
    let new = FootprintFile::from_footprint(Footprint::empty("new"))
        .to_toml_string()
        .expect("encode");
    let sha = commit_revision(dir.path(), &path, &old);
    commit_revision(dir.path(), &path, &new);
    let mut app = open_tab(dir.path(), &path, TabKind::FootprintEditor(path.clone()));
    app.handle_history_restore_clicked(&sha);
    assert!(app.document_state.footprint_editors.contains_key(&path));
    assert_restored_and_committed(&app, &path, &old);
}

#[test]
fn restoring_a_symbol_queues_one_restore_commit() {
    use signex_library::{Symbol, SymbolFile};
    let dir = tempfile::tempdir().expect("tempdir");
    let path: PathBuf = dir.path().join("part.snxsym");
    let old = SymbolFile::from_symbol(Symbol::empty("old"))
        .to_toml_string()
        .expect("encode");
    let new = SymbolFile::from_symbol(Symbol::empty("new"))
        .to_toml_string()
        .expect("encode");
    let sha = commit_revision(dir.path(), &path, &old);
    commit_revision(dir.path(), &path, &new);
    let mut app = open_tab(dir.path(), &path, TabKind::SymbolEditor(path.clone()));
    app.handle_history_restore_clicked(&sha);
    assert!(app.document_state.symbol_editors.contains_key(&path));
    assert_restored_and_committed(&app, &path, &old);
}
