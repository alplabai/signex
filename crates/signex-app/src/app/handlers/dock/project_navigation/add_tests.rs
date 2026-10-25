//! #833 / #852 — the Add Existing / Add New flows name their project by
//! `ProjectId`, do their file work off the update thread, and register the
//! result on the project the dialog was opened for.

use super::add_io;
use crate::app::handlers::menu::export::tests::{add_project, app_workspace};
use crate::app::{Message, ProjectMsg, Signex};
use crate::library::LibraryMessage;

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
fn add_existing_copies_off_thread_and_registers_on_the_original_project() {
    let (mut app, _d1, d2) = two_projects();
    let outside = tempfile::tempdir().expect("outside dir");
    let picked = outside.path().join("Copied.snxsch");
    std::fs::write(&picked, b"x").expect("write picked file");
    let target = app.document_state.projects[0].id;
    let dir1 = std::path::PathBuf::from(&app.document_state.projects[0].data.dir);

    // `update` only plans the copy; nothing touches the disk until the worker
    // runs.
    let _ = app.update(Message::Project(ProjectMsg::AddExistingFilePicked {
        project_id: target,
        paths: Some(vec![picked.clone()]),
    }));
    assert!(!dir1.join("Copied.snxsch").exists(), "no IO inside update");

    let copied = add_io::run_existing(add_io::plan_existing(&dir1, picked));
    assert!(dir1.join("Copied.snxsch").exists());

    // The list is reordered while the copy runs.
    app.document_state.projects.reverse();
    let _ = app.update(Message::Project(ProjectMsg::AddExistingFilesCopied {
        project_id: target,
        results: Ok(vec![copied]),
    }));

    let owner = app
        .document_state
        .projects
        .iter()
        .find(|p| p.id == target)
        .expect("project 1 still open");
    assert_eq!(owner.data.sheets.len(), 1, "project 1 gets the sheet");
    assert!(
        app.document_state
            .projects
            .iter()
            .filter(|p| p.id != target)
            .all(|p| p.data.sheets.is_empty()),
        "project 2 is untouched"
    );
    drop(d2);
}

#[test]
fn add_existing_failures_reach_the_messages_panel() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, d1, _d2) = two_projects();
    let target = app.document_state.projects[0].id;
    let taken = d1.path().join("Taken.snxsch");
    std::fs::write(&taken, b"x").expect("existing file");
    let outside = tempfile::tempdir().expect("outside dir");
    let picked = outside.path().join("Taken.snxsch");
    std::fs::write(&picked, b"y").expect("picked file");

    let outcome = add_io::run_existing(add_io::plan_existing(d1.path(), picked));
    assert!(
        outcome.is_err(),
        "an existing destination is not overwritten"
    );
    assert_eq!(std::fs::read(&taken).expect("read"), b"x");

    let _ = app.update(Message::Project(ProjectMsg::AddExistingFilesCopied {
        project_id: target,
        results: Ok(vec![outcome]),
    }));
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("Add Existing")
                && e.message.contains("destination already exists")),
        "the failure must reach the Messages panel"
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
    assert!(!new_sheet.exists(), "no IO inside update");

    // The worker writes the blank sheet; the list is reordered once more.
    add_io::write_blank_schematic(&new_sheet).expect("blank sheet written");
    app.document_state.projects.reverse();
    let _ = app.update(Message::Project(ProjectMsg::AddNewSchematicWritten {
        project_id: target,
        path: new_sheet.clone(),
        result: Ok(()),
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

#[test]
fn a_failed_blank_schematic_write_reaches_the_messages_panel() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, d1, _d2) = two_projects();
    let target = app.document_state.projects[0].id;
    let existing = d1.path().join("Here.snxsch");
    std::fs::write(&existing, b"keep").expect("existing file");

    let result = add_io::write_blank_schematic(&existing);
    assert_eq!(std::fs::read(&existing).expect("read"), b"keep");
    let _ = app.update(Message::Project(ProjectMsg::AddNewSchematicWritten {
        project_id: target,
        path: existing,
        result,
    }));

    assert!(app.document_state.projects[0].data.sheets.is_empty());
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("Add New Schematic")
                && e.message.contains("destination already exists")),
        "the failure must reach the Messages panel"
    );
}

#[test]
fn a_library_file_lands_on_the_project_it_was_started_for_not_the_active_one() {
    let (mut app, d1, d2) = two_projects();
    let target = app.document_state.projects[0].id;
    // A different project is the sticky active one when the write completes,
    // and the file lives outside both project directories so only the id can
    // pick the project.
    app.document_state.active_project = Some(app.document_state.projects[1].id);
    let outside = tempfile::tempdir().expect("outside dir");
    let lib = outside.path().join("Mine.snxsym");
    std::fs::write(&lib, b"").expect("file");

    let _ = app.update(Message::Library(LibraryMessage::AddLibraryFileWritten {
        project_id: target,
        path: lib,
        result: Ok(()),
    }));
    drop((d1, d2));

    let libs = |i: usize| app.document_state.projects[i].data.libraries.len();
    assert_eq!(libs(0), 1, "the starting project registers the library");
    assert_eq!(libs(1), 0, "the active project does not");
}
