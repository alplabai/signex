//! #833 — the Add Existing / Add New completions name their project by
//! `ProjectId`, so a project closed or reordered while the file dialog was up
//! can never redirect the add onto a different project.

use crate::app::handlers::menu::export::tests::{add_project, app_workspace};
use crate::app::{Message, ProjectMsg, Signex};

/// Two projects on disk; returns the app plus each project's directory.
fn two_projects() -> (Signex, tempfile::TempDir, tempfile::TempDir) {
    let d1 = tempfile::tempdir().expect("tempdir 1");
    let d2 = tempfile::tempdir().expect("tempdir 2");
    let mut app = app_workspace(&d1.path().to_string_lossy(), &[]);
    add_project(&mut app, &d2.path().to_string_lossy(), &[]);
    (app, d1, d2)
}

#[test]
fn a_project_closed_before_add_existing_completes_does_not_touch_the_wrong_project() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, d1, d2) = two_projects();
    let picked = d1.path().join("x.snxsch");
    std::fs::write(&picked, b"x").expect("write picked file");
    let target = app.document_state.projects[0].id;

    // The dialog opened for project 1; project 1 is closed while it is up, so
    // project 2 slides into index 0.
    app.document_state.projects.remove(0);
    let _ = app.update(Message::Project(ProjectMsg::AddExistingFilePicked {
        project_id: target,
        paths: Some(vec![picked]),
    }));

    assert!(
        app.document_state.projects[0].data.sheets.is_empty(),
        "the surviving project must not gain the sheet"
    );
    assert!(
        !d2.path().join("x.snxsch").exists(),
        "nothing may be copied into the surviving project's directory"
    );
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("Add Existing") && e.message.contains(&target.to_string())),
        "the abort must reach the Messages panel"
    );
}

#[test]
fn a_project_reordered_before_add_new_completes_still_gets_its_own_sheet() {
    let (mut app, d1, d2) = two_projects();
    let target = app.document_state.projects[0].id;
    let new_sheet = d1.path().join("Fresh.snxsch");

    // The dialog opened for project 1; the list is reordered while it is up.
    app.document_state.projects.reverse();
    let _ = app.update(Message::Project(ProjectMsg::AddNewSchematicPicked {
        project_id: target,
        path: Some(new_sheet.clone()),
    }));
    drop(d2);

    let owner = app
        .document_state
        .projects
        .iter()
        .find(|p| p.id == target)
        .expect("project 1 still open");
    assert_eq!(owner.data.sheets.len(), 1, "project 1 gets its new sheet");
    assert!(new_sheet.exists());
    let other = app
        .document_state
        .projects
        .iter()
        .find(|p| p.id != target)
        .expect("project 2 still open");
    assert!(other.data.sheets.is_empty(), "project 2 is untouched");
}
