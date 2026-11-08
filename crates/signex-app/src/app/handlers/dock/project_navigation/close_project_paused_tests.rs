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

#[test]
fn closing_a_project_drops_its_saved_state_cursor() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    let id = app.document_state.projects[0].id;
    let path = app.document_state.projects[0].path.clone();
    app.document_state.note_project_clean_state(id);
    assert!(app.document_state.has_project_saved_state(&path));

    let refs = app.resolve_sheet_refs_blocking().expect("refs");
    let _ = app.execute_close_project(id, &refs, &[]);

    assert!(!app.document_state.has_project_saved_state(&path));
}

/// #2172: closing a project forgets the cached document-id heads of its own
/// files only; a file another open project lists in the same folder keeps its.
#[test]
fn closing_a_project_forgets_the_heads_of_its_own_files_and_no_others() {
    use signex_types::document_id::DocumentId;
    let dir = tempfile::tempdir().expect("tempdir");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &[]);
    let id = app.document_state.projects[0].id;
    let own: Vec<_> = crate::app::document_ids::referenced_files(&app.document_state.projects[0])
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    assert!(!own.is_empty(), "fixture: the project lists a file");
    let other = dir.path().join("another-projects-sheet.snxsch");
    let some_id = DocumentId::new();
    for path in own.iter().chain([&other]) {
        app.document_state
            .document_ids
            .set_header(path.clone(), Some(some_id), None);
    }

    let refs = app.resolve_sheet_refs_blocking().expect("refs");
    let _ = app.execute_close_project(id, &refs, &[]);

    for path in &own {
        assert_eq!(app.document_state.document_ids.header(path), None);
    }
    assert!(app.document_state.document_ids.header(&other).is_some());
}
