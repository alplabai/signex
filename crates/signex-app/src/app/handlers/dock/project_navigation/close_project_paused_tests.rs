//! #1205 — closing a project forgets that its auto-commit was paused.

use crate::app::handlers::menu::export::tests::app_workspace;

#[test]
fn closing_a_project_lifts_its_paused_auto_commit_mark() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    let root = std::path::PathBuf::from(&app.document_state.projects[0].data.dir);
    app.document_state.paused_git_projects.insert(root);
    let id = app.document_state.projects[0].id;

    let refs = app.resolve_sheet_refs_blocking().expect("refs");
    let _ = app.execute_close_project(id, &refs, &[]);

    assert!(app.document_state.projects.iter().all(|p| p.id != id));
    assert!(app.document_state.paused_git_projects.is_empty());
}
