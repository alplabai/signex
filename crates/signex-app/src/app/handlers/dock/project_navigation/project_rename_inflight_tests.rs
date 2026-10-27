//! #1323 — a project folder rename is refused (to be retried) while a write
//! still runs into the folder, the writes and mounts that would race it are
//! refused while it runs, and it carries the recovery copies of the open
//! documents to the new folder.

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
    assert_eq!(
        fx.app.document_state.project_writes,
        std::slice::from_ref(&new_sheet)
    );
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
    assert_eq!(fx.app.document_state.project_writes.len(), 1);
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

/// How often the Messages panel logged a line naming `path` (the ring is
/// shared between tests, so compare counts with a test-unique path).
fn warnings_about(path: &std::path::Path) -> usize {
    crate::diagnostics::logged_count(&path.display().to_string())
}

#[test]
fn a_failed_export_write_still_releases_the_rename_guard() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    // A directory where the file should go: the write fails.
    let out = fx.folder.join("outputs");
    std::fs::create_dir_all(out.join("blocker")).expect("blocking dir");
    let task = fx.app.write_export(
        out.clone(),
        b"x".to_vec(),
        "netlist",
        String::new(),
        None,
        Vec::new(),
    );
    assert_eq!(fx.app.document_state.project_writes, [out]);
    run_task(&mut fx.app, task);
    assert!(fx.app.document_state.project_writes.is_empty());
    assert!(
        fx.app.document_state.error_notice.is_some(),
        "failure shown"
    );
}

#[test]
fn the_erc_report_write_holds_the_folder_rename_until_it_lands() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    // ERC has run, with nothing to report.
    fx.app
        .ui_state
        .erc_violations_by_path
        .insert(fx.sheet.clone(), Vec::new());
    let report = fx.folder.join("erc.csv");
    let task = fx.app.handle_erc_report_path_chosen(Ok(report.clone()));
    assert_eq!(
        fx.app.document_state.project_writes,
        std::slice::from_ref(&report)
    );
    let rename = rename_foo_to_bar(&mut fx);
    run_task(&mut fx.app, rename);
    assert!(rename_error(&fx).contains("being written"));
    run_task(&mut fx.app, task);
    assert!(fx.app.document_state.project_writes.is_empty());
    assert!(report.is_file());

    let id = fx.app.document_state.projects[0].id;
    fx.app.document_state.project_folder_renames.insert(id);
    let refused = fx.folder.join("erc-2.csv");
    let before = warnings_about(&refused);
    let task = fx.app.handle_erc_report_path_chosen(Ok(refused.clone()));
    assert!(iced_runtime::task::into_stream(task).is_none(), "no write");
    assert!(fx.app.document_state.error_notice.is_some());
    assert_eq!(warnings_about(&refused), before + 1, "logged");
}

#[test]
fn a_new_library_file_is_counted_by_its_path_and_refused_during_a_move() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let id = fx.app.document_state.projects[0].id;
    let lib = fx.folder.join("New.snxsym");

    fx.app.document_state.project_folder_renames.insert(id);
    let before = warnings_about(&lib);
    let task = fx
        .app
        .handle_add_library_symbol_file_picked(id, lib.clone());
    assert!(iced_runtime::task::into_stream(task).is_none(), "no write");
    assert!(fx.app.document_state.project_writes.is_empty());
    assert!(fx.app.document_state.error_notice.take().is_some());
    assert_eq!(warnings_about(&lib), before + 1, "logged");
    fx.app.document_state.project_folder_renames.clear();

    let task = fx
        .app
        .handle_add_library_symbol_file_picked(id, lib.clone());
    assert_eq!(
        fx.app.document_state.project_writes,
        std::slice::from_ref(&lib)
    );
    let rename = rename_foo_to_bar(&mut fx);
    run_task(&mut fx.app, rename);
    assert!(rename_error(&fx).contains("being written"));
    run_task(&mut fx.app, task);
    assert!(fx.app.document_state.project_writes.is_empty());
    assert!(lib.is_file());
}

#[test]
fn a_failed_new_library_file_write_still_releases_the_rename_guard() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let id = fx.app.document_state.projects[0].id;
    let lib = fx.folder.join("Blocked.snxsym");
    std::fs::create_dir_all(lib.join("blocker")).expect("blocking dir");
    let task = fx.app.handle_add_library_symbol_file_picked(id, lib);
    assert_eq!(fx.app.document_state.project_writes.len(), 1);
    run_task(&mut fx.app, task);
    assert!(fx.app.document_state.project_writes.is_empty());
}

#[test]
fn a_write_aimed_at_the_folder_before_the_move_is_refused_after_it() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let task = rename_foo_to_bar(&mut fx);
    run_task(&mut fx.app, task);
    assert!(!fx.folder.exists(), "moved");

    // A save dialog opened on the old folder answers now.
    let (_, export) = export_into(&mut fx);
    assert!(
        iced_runtime::task::into_stream(export).is_none(),
        "no write"
    );
    let notice = fx.app.document_state.error_notice.take().expect("notice");
    assert!(notice.detail.contains("was renamed"), "{}", notice.detail);
    assert!(!fx.folder.exists(), "the old folder was not recreated");
    assert!(fx.app.document_state.project_writes.is_empty());

    // The new folder takes writes.
    let out = root.path().join("Bar").join("outputs").join("netlist.net");
    let task = fx.app.write_export(
        out.clone(),
        b"x".to_vec(),
        "netlist",
        String::new(),
        None,
        Vec::new(),
    );
    run_task(&mut fx.app, task);
    assert!(out.is_file());
}

#[test]
fn a_dot_dot_spelling_of_an_open_document_is_rebased_into_the_new_folder() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let spelled = root
        .path()
        .join("x")
        .join("..")
        .join("Foo")
        .join("main.snxsch");
    let engine = fx
        .app
        .document_state
        .engines
        .remove(&fx.sheet)
        .expect("engine");
    fx.app
        .document_state
        .engines
        .insert(spelled.clone(), engine);
    fx.app.document_state.tabs[0].path = spelled;

    let task = rename_foo_to_bar(&mut fx);
    run_task(&mut fx.app, task);
    let new_sheet = root.path().join("Bar").join("main.snxsch");
    assert_eq!(fx.app.document_state.tabs[0].path, new_sheet);
    assert!(fx.app.document_state.engines.contains_key(&new_sheet));
}

#[test]
fn a_failed_recovery_copy_move_keeps_the_copy_until_a_fresh_one_exists() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let recovery = root.path().join("recovery");
    fx.app
        .document_state
        .autosave
        .set_dir(Some(recovery.clone()));
    let old_sheet = fx.sheet.clone();
    let new_sheet = root.path().join("Bar").join("main.snxsch");
    crate::autosave::sidecar::write(&recovery, &old_sheet, b"unsaved").expect("sidecar");
    let stamp = crate::autosave::DocStamp::of_bytes(b"unsaved");
    let autosave = &mut fx.app.document_state.autosave;
    autosave.begin(&old_sheet);
    autosave.finish(&old_sheet, stamp, true);
    // A non-empty directory where the moved copy should go: the move fails.
    let new_copy = crate::autosave::sidecar::sidecar_path(&recovery, &new_sheet);
    std::fs::create_dir_all(new_copy.join("blocker")).expect("blocking dir");
    let old_copy = crate::autosave::sidecar::sidecar_path(&recovery, &old_sheet);

    let task = rename_foo_to_bar(&mut fx);
    run_task(&mut fx.app, task);
    assert!(new_sheet.is_file(), "the folder moved");
    assert!(old_copy.is_file(), "the copy stayed under the old key");
    assert!(
        fx.app
            .document_state
            .autosave
            .needs_write(&new_sheet, stamp),
        "the next tick writes a fresh copy at the new key"
    );

    // The sweep keeps the only recovery copy while none exists at the new key.
    let sweep = fx.app.autosave_sweep();
    run_task(&mut fx.app, sweep);
    assert!(old_copy.is_file(), "not swept");

    // Once a fresh copy is written, the old one goes.
    let autosave = &mut fx.app.document_state.autosave;
    autosave.begin(&new_sheet);
    autosave.finish(&new_sheet, stamp, true);
    let sweep = fx.app.autosave_sweep();
    run_task(&mut fx.app, sweep);
    assert!(!old_copy.exists(), "the stale copy was removed");
}

#[test]
fn a_git_commit_asked_for_during_the_move_runs_after_it() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", true);
    std::fs::write(&fx.sheet, b"edited").expect("edit");
    let task = rename_foo_to_bar(&mut fx);
    let sheet = fx.sheet.clone();
    fx.app.commit_save_to_project_git(&sheet, "Save main");
    let ds = &fx.app.document_state;
    assert_eq!(ds.deferred_git_commits.len(), 1, "deferred, not dropped");
    assert!(ds.pending_git_commits.is_empty());

    run_task(&mut fx.app, task);
    let new_dir = root.path().join("Bar");
    assert!(fx.app.document_state.deferred_git_commits.is_empty());
    let repo = git2::Repository::open(&new_dir).expect("repo");
    let head = repo.head().expect("head").peel_to_commit().expect("commit");
    assert_eq!(
        head.message().expect("utf-8").lines().next(),
        Some("Save main"),
        "the deferred commit ran in the moved project, after the rename"
    );
}
