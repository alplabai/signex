//! #1323 — a project folder rename waits for every write still running into
//! the folder, refuses to start when a write or mount would race it, and
//! carries the recovery copies of the open documents to the new folder.

use super::project_rename_tests::{Fixture, fixture, rename_error, run_task, submit_rename};

fn rename_foo_to_bar(fx: &mut Fixture) -> iced::Task<crate::app::Message> {
    let file = fx.folder.join("Foo.snxprj");
    submit_rename(&mut fx.app, &file, "Bar")
}

fn export_into(fx: &mut Fixture) -> (std::path::PathBuf, iced::Task<crate::app::Message>) {
    let out = fx.folder.join("outputs").join("netlist.net");
    let task = fx.app.write_export(
        out.clone(),
        b"x".to_vec(),
        "netlist",
        String::new(),
        None,
        Vec::new(),
    );
    (out, task)
}

#[test]
fn a_running_export_write_holds_the_folder_rename_until_it_lands() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let (out, export) = export_into(&mut fx);
    assert_eq!(fx.app.document_state.project_writes.len(), 1);

    let task = rename_foo_to_bar(&mut fx);
    run_task(&mut fx.app, task);
    assert!(fx.folder.join("Foo.snxprj").is_file(), "nothing moved");
    assert!(
        rename_error(&fx).contains("being written"),
        "{}",
        rename_error(&fx)
    );

    run_task(&mut fx.app, export);
    assert!(fx.app.document_state.project_writes.is_empty());
    assert!(out.is_file());
    let task = rename_foo_to_bar(&mut fx);
    run_task(&mut fx.app, task);
    assert!(
        root.path()
            .join("Bar")
            .join("outputs")
            .join("netlist.net")
            .is_file(),
        "the export moved with the folder"
    );
}

#[test]
fn add_new_schematic_is_tracked_and_refused_during_a_move() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let id = fx.app.document_state.projects[0].id;
    let new_sheet = fx.folder.join("extra.snxsch");

    fx.app.document_state.project_folder_renames.insert(id);
    let task = fx
        .app
        .handle_add_new_schematic_picked(id, Some(new_sheet.clone()));
    assert!(iced_runtime::task::into_stream(task).is_none(), "no write");
    assert!(fx.app.document_state.project_writes.is_empty());
    assert!(fx.app.document_state.error_notice.take().is_some());
    fx.app.document_state.project_folder_renames.clear();

    let task = fx
        .app
        .handle_add_new_schematic_picked(id, Some(new_sheet.clone()));
    assert_eq!(fx.app.document_state.project_writes, [id]);
    let rename = rename_foo_to_bar(&mut fx);
    run_task(&mut fx.app, rename);
    assert!(rename_error(&fx).contains("being written"));
    run_task(&mut fx.app, task);
    assert!(fx.app.document_state.project_writes.is_empty());
    assert!(new_sheet.is_file());
}

#[test]
fn add_existing_copy_is_tracked_and_refused_during_a_move() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let id = fx.app.document_state.projects[0].id;
    let outside = root.path().join("Outside.snxsch");
    std::fs::write(&outside, b"x").expect("source");

    fx.app.document_state.project_folder_renames.insert(id);
    let task = fx
        .app
        .handle_add_existing_file_picked(id, Some(vec![outside.clone()]));
    assert!(iced_runtime::task::into_stream(task).is_none(), "no copy");
    assert!(!fx.folder.join("Outside.snxsch").exists());
    fx.app.document_state.project_folder_renames.clear();

    let task = fx
        .app
        .handle_add_existing_file_picked(id, Some(vec![outside]));
    assert_eq!(fx.app.document_state.project_writes, [id]);
    let rename = rename_foo_to_bar(&mut fx);
    run_task(&mut fx.app, rename);
    assert!(rename_error(&fx).contains("being written"));
    run_task(&mut fx.app, task);
    assert!(fx.app.document_state.project_writes.is_empty());
}

#[test]
fn a_library_browser_open_in_a_moving_project_is_refused() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let id = fx.app.document_state.projects[0].id;
    fx.app.document_state.project_folder_renames.insert(id);
    let lib = fx.folder.join("parts").join("p.snxlib");
    let task = fx.app.handle_open_library_browser(lib.clone());
    assert!(iced_runtime::task::into_stream(task).is_none(), "no mount");
    assert!(!fx.app.library.pending_mounts.contains_key(&lib));
    assert!(fx.app.document_state.error_notice.is_some());
}

#[test]
fn history_restore_is_refused_in_a_moving_project() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let id = fx.app.document_state.projects[0].id;
    fx.app.document_state.dirty_paths.clear();
    fx.app.document_state.project_folder_renames.insert(id);
    fx.app.handle_history_restore_clicked("abc123");
    let notice = fx.app.document_state.error_notice.take().expect("notice");
    assert!(notice.detail.contains("being renamed"), "{}", notice.detail);
}

#[test]
fn annotate_and_reset_refuse_up_front_in_a_moving_project() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let id = fx.app.document_state.projects[0].id;
    fx.app.document_state.project_folder_renames.insert(id);
    let before = fx.app.document_state.dirty_paths.clone();

    let _ = fx
        .app
        .handle_annotate(signex_engine::AnnotateMode::Incremental);
    let notice = fx.app.document_state.error_notice.take().expect("notice");
    assert!(notice.detail.contains("being renamed"), "{}", notice.detail);
    assert!(notice.detail.starts_with("Annotate"));

    let _ = fx.app.handle_reset_duplicate_designators();
    let notice = fx.app.document_state.error_notice.take().expect("notice");
    assert!(notice.detail.contains("being renamed"), "{}", notice.detail);
    assert!(notice.detail.starts_with("Reset Duplicate"));
    assert_eq!(fx.app.document_state.dirty_paths, before, "nothing ran");
}

#[test]
fn the_recovery_copy_of_a_dirty_sheet_follows_the_folder() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let recovery = root.path().join("recovery");
    fx.app
        .document_state
        .autosave
        .set_dir(Some(recovery.clone()));
    let old_sheet = fx.sheet.clone();
    crate::autosave::sidecar::write(&recovery, &old_sheet, b"unsaved").expect("sidecar");
    let stamp = crate::autosave::DocStamp::of_bytes(b"unsaved");
    fx.app.document_state.autosave.begin(&old_sheet);
    fx.app
        .document_state
        .autosave
        .finish(&old_sheet, stamp, true);

    let task = rename_foo_to_bar(&mut fx);
    run_task(&mut fx.app, task);

    let new_sheet = root.path().join("Bar").join("main.snxsch");
    assert!(new_sheet.is_file());
    assert!(!crate::autosave::sidecar::sidecar_path(&recovery, &old_sheet).exists());
    assert_eq!(
        std::fs::read(crate::autosave::sidecar::sidecar_path(
            &recovery, &new_sheet
        ))
        .expect("moved sidecar"),
        b"unsaved"
    );
}
