//! The rollback of a library rename (#739, #859): the reverse patch of the
//! saved `.snxprj` and the move back run on a worker, and the completion
//! reports what is left.

use super::*;

fn rollback_job(project: &Path, old: &Path, new: &Path, patched: bool) -> LibraryRollbackJob {
    LibraryRollbackJob {
        old: old.to_path_buf(),
        new: new.to_path_buf(),
        project_path: project.to_path_buf(),
        patched,
    }
}

fn app_with_open_project(project_path: &Path) -> Signex {
    let (mut app, _task) = Signex::new();
    let id = app.document_state.mint_project_id();
    app.document_state
        .projects
        .push(crate::app::state::LoadedProject {
            id,
            path: project_path.to_path_buf(),
            data: serde_json::from_value(serde_json::json!({
                "name": "proj",
                "dir": "",
                "schematic_root": null,
                "pcb_file": null,
            }))
            .expect("project data"),
            pending_libraries: Default::default(),
            case_fold: Default::default(),
            case_probed: false,
            case_decided: false,
        });
    app
}

/// A corrupt project file makes `retarget_library_entry` fail (it parses
/// before it writes), which is how the reverse patch fails here.
#[test]
fn a_failed_reverse_patch_dirties_the_stale_project_once_the_file_is_back() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_path = tmp.path().join("proj.snxprj");
    std::fs::write(&project_path, b"{ not json").unwrap();
    let old_path = tmp.path().join("alpha.snxlib");
    let new_path = tmp.path().join("alpha2.snxlib");
    std::fs::write(&new_path, b"x").unwrap();

    let (patch_undo, moved_back) = rollback_job(&project_path, &old_path, &new_path, true).run();

    let suffix = patch_undo.unwrap_err();
    assert!(suffix.contains("Save the project"), "got: {suffix}");
    assert!(moved_back.is_ok(), "the file still moves back");
    assert!(old_path.exists() && !new_path.exists());

    // The file is back, so the project is dirtied: a save then writes the
    // old-path entry the saved file no longer lists.
    let mut app = app_with_open_project(&project_path);
    app.report_library_rollback(
        &old_path,
        &new_path,
        "r",
        moved_back,
        Some((project_path.clone(), suffix)),
    );
    assert!(app.document_state.is_document_dirty(&project_path));
}

#[test]
fn the_stale_project_is_dirtied_only_when_the_file_moves_back() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_path = tmp.path().join("proj.snxprj");
    let old_path = tmp.path().join("alpha.snxlib");
    let new_path = tmp.path().join("alpha2.snxlib");
    let mut app = app_with_open_project(&project_path);
    let stale = || Some((project_path.clone(), "; still lists it".to_string()));

    app.report_library_rollback(&old_path, &new_path, "r", Ok(()), stale());
    assert!(app.document_state.is_document_dirty(&project_path));

    app.document_state.mark_documents_clean_where(|_| true);
    app.report_library_rollback(
        &old_path,
        &new_path,
        "r",
        Err("denied".to_string()),
        stale(),
    );
    assert!(!app.document_state.has_dirty_documents());
    assert!(
        app.document_state.error_notice.is_some(),
        "a stranded library raises the card"
    );
}

#[test]
fn a_project_closed_meanwhile_is_not_dirtied_by_the_rollback() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_path = tmp.path().join("proj.snxprj");
    let (mut app, _task) = Signex::new();

    app.report_library_rollback(
        &tmp.path().join("a.snxlib"),
        &tmp.path().join("b.snxlib"),
        "r",
        Ok(()),
        Some((project_path, "; still lists it".to_string())),
    );

    assert!(!app.document_state.has_dirty_documents());
}

#[test]
fn a_rollback_of_an_unsaved_project_moves_the_file_back() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_path = tmp.path().join("missing.snxprj");
    let old_path = tmp.path().join("alpha.snxlib");
    let new_path = tmp.path().join("alpha2.snxlib");
    std::fs::write(&new_path, b"x").unwrap();

    let (patch_undo, moved_back) = rollback_job(&project_path, &old_path, &new_path, true).run();

    assert!(patch_undo.is_ok() && moved_back.is_ok());
}
