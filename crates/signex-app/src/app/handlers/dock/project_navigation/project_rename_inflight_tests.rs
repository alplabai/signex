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
    fx.app.document_state.mark_documents_clean_where(|_| true);
    fx.app.document_state.project_folder_renames.insert(id);
    drop(fx.app.handle_history_restore_clicked("abc123"));
    let notice = fx.app.document_state.error_notice.take().expect("notice");
    assert!(notice.detail.contains("being renamed"), "{}", notice.detail);
}

#[test]
fn annotate_and_reset_refuse_up_front_in_a_moving_project() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let id = fx.app.document_state.projects[0].id;
    fx.app.document_state.project_folder_renames.insert(id);
    let before = fx.app.document_state.dirty_document_paths();

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
    assert_eq!(
        fx.app.document_state.dirty_document_paths(),
        before,
        "nothing ran"
    );
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

/// A recovery dir with a copy of the fixture's sheet under its current key,
/// and a non-empty directory where the moved copy would go, so the sidecar
/// move after `Foo` -> `Bar` fails. Returns (old copy, new copy, new sheet).
fn block_the_copy_move(
    fx: &mut Fixture,
    root: &std::path::Path,
) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let recovery = root.join("recovery");
    fx.app
        .document_state
        .autosave
        .set_dir(Some(recovery.clone()));
    let new_sheet = root.join("Bar").join("main.snxsch");
    crate::autosave::sidecar::write(&recovery, &fx.sheet, b"unsaved").expect("sidecar");
    let new_copy = crate::autosave::sidecar::sidecar_path(&recovery, &new_sheet);
    std::fs::create_dir_all(new_copy.join("blocker")).expect("blocking dir");
    let old_copy = crate::autosave::sidecar::sidecar_path(&recovery, &fx.sheet);
    (old_copy, new_copy, new_sheet)
}

fn unblock(new_copy: &std::path::Path) {
    std::fs::remove_dir_all(new_copy).expect("unblock");
}

#[test]
fn a_failed_recovery_copy_move_is_retried_until_the_copy_follows() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let (old_copy, new_copy, new_sheet) = block_the_copy_move(&mut fx, root.path());
    let stamp = crate::autosave::DocStamp::of_bytes(b"unsaved");
    let autosave = &mut fx.app.document_state.autosave;
    autosave.begin(&fx.sheet);
    autosave.finish(&fx.sheet, stamp, true);

    let task = rename_foo_to_bar(&mut fx);
    run_task(&mut fx.app, task);
    assert!(new_sheet.is_file(), "the folder moved");
    assert!(old_copy.is_file(), "the copy stayed under the old key");
    let newer = crate::autosave::DocStamp::of_bytes(b"newer");
    assert!(
        !fx.app
            .document_state
            .autosave
            .needs_write(&new_sheet, newer),
        "no write lands at the new key before the copy does"
    );

    // Still locked on the next tick: the copy stays put.
    let retry = fx.app.autosave_flush_moves();
    run_task(&mut fx.app, retry);
    assert!(old_copy.is_file());

    unblock(&new_copy);
    let retry = fx.app.autosave_flush_moves();
    run_task(&mut fx.app, retry);
    assert!(!old_copy.exists());
    assert_eq!(std::fs::read(&new_copy).expect("moved"), b"unsaved");
    assert!(
        fx.app
            .document_state
            .autosave
            .needs_write(&new_sheet, newer)
    );
}

#[test]
fn a_kept_earlier_run_copy_whose_move_failed_is_still_restorable() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let (old_copy, new_copy, new_sheet) = block_the_copy_move(&mut fx, root.path());
    // "Decide later": the copy is not one autosave wrote, so not in `written`.
    fx.app.document_state.autosave.defer(&fx.sheet);

    let task = rename_foo_to_bar(&mut fx);
    run_task(&mut fx.app, task);
    assert!(old_copy.is_file(), "the copy is not dropped");

    unblock(&new_copy);
    let retry = fx.app.autosave_flush_moves();
    run_task(&mut fx.app, retry);
    assert!(!old_copy.exists());
    assert_eq!(
        std::fs::read(&new_copy).expect("where Restore reads it"),
        b"unsaved"
    );
    let stamp = crate::autosave::DocStamp::of_bytes(b"x");
    assert!(
        !fx.app
            .document_state
            .autosave
            .needs_write(&new_sheet, stamp),
        "still kept at the new path"
    );
}

#[test]
fn a_failed_copy_move_of_a_saved_document_removes_the_old_copy() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let (old_copy, new_copy, new_sheet) = block_the_copy_move(&mut fx, root.path());
    let stamp = crate::autosave::DocStamp::of_bytes(b"unsaved");
    let autosave = &mut fx.app.document_state.autosave;
    autosave.begin(&fx.sheet);
    autosave.finish(&fx.sheet, stamp, true);
    let task = rename_foo_to_bar(&mut fx);
    run_task(&mut fx.app, task);
    assert!(old_copy.is_file());

    // Saved since: nothing left to recover.
    fx.app.document_state.autosave.saved(&new_sheet);
    let retry = fx.app.autosave_flush_moves();
    run_task(&mut fx.app, retry);
    assert!(!old_copy.exists(), "the obsolete copy was removed");
    assert!(new_copy.is_dir(), "nothing was moved onto the new key");
}

#[test]
fn a_re_created_old_folder_is_usable_again_after_the_move() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let task = rename_foo_to_bar(&mut fx);
    run_task(&mut fx.app, task);
    let again = fx.folder.join("again.snxsch");
    assert!(fx.app.refuse_while_project_moves(&again, "open").is_err());

    // A New Project there, or the folder made by hand.
    std::fs::create_dir(&fx.folder).expect("re-create");
    assert!(fx.app.refuse_while_project_moves(&again, "open").is_ok());
    let (out, export) = export_into(&mut fx);
    run_task(&mut fx.app, export);
    assert!(out.is_file(), "the export wrote into the re-created folder");
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

#[test]
fn restoring_an_offered_copy_whose_move_is_owed_keeps_it_offered_and_safe() {
    use crate::app::Message;
    use crate::autosave::{AutosaveMsg, RecoveryChoice, RecoveryOffer};
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let (old_copy, new_copy, new_sheet) = block_the_copy_move(&mut fx, root.path());
    // An earlier run's copy on offer: not one autosave wrote.
    fx.app.document_state.autosave.offer(RecoveryOffer {
        path: fx.sheet.clone(),
        age_secs: 60,
        older_than_file: false,
    });
    let task = rename_foo_to_bar(&mut fx);
    run_task(&mut fx.app, task);
    assert!(old_copy.is_file(), "the move failed");

    let answer = |fx: &mut Fixture| {
        let task = fx.app.update(Message::Autosave(AutosaveMsg::Recovery(
            RecoveryChoice::Restore,
        )));
        run_task(&mut fx.app, task);
    };
    answer(&mut fx);
    let notice = fx.app.document_state.error_notice.take().expect("notice");
    assert!(
        notice.detail.contains("still being moved"),
        "{}",
        notice.detail
    );
    let offer = fx
        .app
        .document_state
        .autosave
        .current_offer()
        .expect("offered");
    assert_eq!(offer.path, new_sheet);

    unblock(&new_copy);
    let retry = fx.app.autosave_flush_moves();
    run_task(&mut fx.app, retry);
    assert!(new_copy.is_file(), "the copy followed");

    // Still on offer at the new path, so an autosave tick of the dirty sheet
    // must not write over it.
    fx.app.ui_state.autosave_enabled = true;
    let tick = fx.app.update(Message::Autosave(AutosaveMsg::Ticked));
    run_task(&mut fx.app, tick);
    assert_eq!(
        std::fs::read(&new_copy).expect("copy"),
        b"unsaved",
        "the earlier-run copy survived"
    );
}

#[test]
fn discard_all_keeps_a_kept_earlier_run_copy_whose_move_is_owed() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let (old_copy, new_copy, _) = block_the_copy_move(&mut fx, root.path());
    fx.app.document_state.autosave.defer(&fx.sheet);
    let task = rename_foo_to_bar(&mut fx);
    run_task(&mut fx.app, task);
    assert!(old_copy.is_file(), "the move failed");

    let discard = fx.app.autosave_discard_all();
    run_task(&mut fx.app, discard);
    assert!(old_copy.is_file(), "an earlier run's copy is not discarded");
    assert!(
        fx.app.document_state.autosave.has_owed_moves(),
        "its move is still owed, not dropped"
    );

    // The quit runs owed moves before it waits.
    unblock(&new_copy);
    let quit = fx.app.wait_for_autosave_then(iced::Task::none());
    run_task(&mut fx.app, quit);
    assert_eq!(std::fs::read(&new_copy).expect("moved"), b"unsaved");
}

#[test]
fn a_rename_is_refused_while_a_recovery_check_still_runs() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    fx.app.document_state.autosave.begin_check(&fx.sheet);

    let task = rename_foo_to_bar(&mut fx);
    run_task(&mut fx.app, task);
    assert!(fx.folder.join("Foo.snxprj").is_file(), "the folder stayed");
    assert!(
        rename_error(&fx).contains("still opening"),
        "{}",
        rename_error(&fx)
    );

    fx.app.ui_state.rename_dialog = Some(crate::app::RenameDialogState {
        project_id: None,
        target_path: fx.sheet.clone(),
        tree_path: vec![0, 0],
        buffer: "other.snxsch".to_string(),
        error: None,
        is_project_rename: false,
    });
    let task = fx
        .app
        .update(crate::app::Message::Rename(crate::app::RenameMsg::Submit));
    run_task(&mut fx.app, task);
    assert!(fx.sheet.is_file(), "the sheet kept its name");
    assert!(
        rename_error(&fx).contains("still opening"),
        "{}",
        rename_error(&fx)
    );

    // Once the check has reported, the rename runs.
    fx.app.document_state.autosave.finish_check(&fx.sheet);
    let task = rename_foo_to_bar(&mut fx);
    run_task(&mut fx.app, task);
    assert!(root.path().join("Bar").join("Bar.snxprj").is_file());
}

#[test]
fn a_restore_read_that_a_rename_overtook_re_offers_the_copy_at_the_new_path() {
    use crate::app::Message;
    use crate::autosave::{AutosaveMsg, RecoveryChoice, RecoveryOffer};
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let recovery = root.path().join("recovery");
    fx.app
        .document_state
        .autosave
        .set_dir(Some(recovery.clone()));
    crate::autosave::sidecar::write(&recovery, &fx.sheet, b"unsaved").expect("sidecar");
    fx.app.document_state.mark_documents_clean_where(|_| true);
    fx.app.document_state.autosave.offer(RecoveryOffer {
        path: fx.sheet.clone(),
        age_secs: 60,
        older_than_file: false,
    });
    // Restore issues the read, and the folder rename lands before it runs.
    let restore = fx.app.update(Message::Autosave(AutosaveMsg::Recovery(
        RecoveryChoice::Restore,
    )));
    let task = rename_foo_to_bar(&mut fx);
    run_task(&mut fx.app, task);
    let new_sheet = root.path().join("Bar").join("main.snxsch");
    let new_copy = crate::autosave::sidecar::sidecar_path(&recovery, &new_sheet);
    assert!(new_copy.is_file(), "the copy followed the sheet");

    run_task(&mut fx.app, restore);
    let notice = fx.app.document_state.error_notice.take().expect("notice");
    assert!(notice.detail.contains("renamed while"), "{}", notice.detail);
    let offer = fx
        .app
        .document_state
        .autosave
        .current_offer()
        .expect("offered again");
    assert_eq!(offer.path, new_sheet);
    assert_eq!(std::fs::read(&new_copy).expect("copy"), b"unsaved");
}

#[test]
fn a_folder_rename_is_refused_while_a_symbol_editor_recovery_check_still_runs() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    fx.app
        .document_state
        .autosave
        .set_dir(Some(root.path().join("recovery")));
    let symbol = fx.folder.join("Part.snxsym");
    let text = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("Part"))
        .to_toml_string()
        .expect("encode");
    std::fs::write(&symbol, text).expect("symbol file");

    // The open reads the file off-thread, then starts the check; until the
    // check reports the folder must not move. Deliver the read only, and
    // keep the check it starts unrun.
    let read = fx.app.handle_open_primitive(symbol.clone());
    let check = {
        use futures::StreamExt;
        let mut stream = iced_runtime::task::into_stream(read).expect("a read");
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let messages = runtime.block_on(async {
            let mut out = Vec::new();
            while let Some(action) = stream.next().await {
                if let iced_runtime::Action::Output(message) = action {
                    out.push(message);
                }
            }
            out
        });
        iced::Task::batch(messages.into_iter().map(|m| fx.app.update(m)))
    };
    assert!(
        fx.app
            .document_state
            .autosave
            .checking_paths()
            .any(|p| *p == symbol),
        "the symbol editor's check is held"
    );
    let task = rename_foo_to_bar(&mut fx);
    run_task(&mut fx.app, task);
    assert!(symbol.is_file(), "the folder stayed");
    assert!(
        rename_error(&fx).contains("still opening"),
        "{}",
        rename_error(&fx)
    );

    run_task(&mut fx.app, check);
    assert!(
        fx.app
            .document_state
            .autosave
            .checking_paths()
            .next()
            .is_none()
    );
    let task = rename_foo_to_bar(&mut fx);
    run_task(&mut fx.app, task);
    assert!(root.path().join("Bar").join("Part.snxsym").is_file());
}

fn set_recovery(fx: &mut Fixture, root: &std::path::Path) -> std::path::PathBuf {
    let recovery = root.join("recovery");
    fx.app
        .document_state
        .autosave
        .set_dir(Some(recovery.clone()));
    recovery
}

#[test]
fn renaming_a_closed_sheet_carries_its_earlier_run_copy() {
    use crate::autosave::sidecar::sidecar_path;
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let recovery = set_recovery(&mut fx, root.path());
    let closed = fx.folder.join("closed.snxsch");
    std::fs::write(&closed, b"sheet").expect("closed sheet");
    crate::autosave::sidecar::write(&recovery, &closed, b"crash edits").expect("copy");

    fx.app.ui_state.rename_dialog = Some(crate::app::RenameDialogState {
        project_id: None,
        target_path: closed.clone(),
        tree_path: vec![0, 0],
        buffer: "renamed.snxsch".to_string(),
        error: None,
        is_project_rename: false,
    });
    let task = fx
        .app
        .update(crate::app::Message::Rename(crate::app::RenameMsg::Submit));
    run_task(&mut fx.app, task);
    let renamed = fx.folder.join("renamed.snxsch");
    assert!(renamed.is_file(), "{:?}", fx.app.ui_state.rename_dialog);
    assert_eq!(
        std::fs::read(sidecar_path(&recovery, &renamed)).expect("copy under the new key"),
        b"crash edits"
    );
    assert!(!sidecar_path(&recovery, &closed).exists());
}

#[test]
fn a_folder_move_carries_the_copies_of_closed_documents() {
    use crate::autosave::sidecar::sidecar_path;
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let recovery = set_recovery(&mut fx, root.path());
    let closed = fx.folder.join("sub").join("b.snxsch");
    std::fs::create_dir_all(closed.parent().expect("sub")).expect("sub");
    std::fs::write(&closed, b"sheet").expect("closed sheet");
    crate::autosave::sidecar::write(&recovery, &closed, b"crash edits").expect("copy");

    let task = rename_foo_to_bar(&mut fx);
    run_task(&mut fx.app, task);
    let moved = root.path().join("Bar").join("sub").join("b.snxsch");
    assert!(moved.is_file(), "the folder moved");
    assert_eq!(
        std::fs::read(sidecar_path(&recovery, &moved)).expect("copy under the new key"),
        b"crash edits"
    );
    assert!(!sidecar_path(&recovery, &closed).exists());
}

#[test]
fn the_stale_dialog_guard_holds_with_an_enclosing_open_project() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    let task = rename_foo_to_bar(&mut fx);
    run_task(&mut fx.app, task);
    // An open project whose folder encloses the moved one.
    let mut outer = fx.app.document_state.projects[0].clone();
    outer.id = crate::app::state::ProjectId::test_id(9_999);
    outer.path = root.path().join("outer.snxprj");
    fx.app.document_state.projects.push(outer);

    let (_, export) = export_into(&mut fx);
    assert!(
        iced_runtime::task::into_stream(export).is_none(),
        "no write"
    );
    assert!(!fx.folder.exists(), "the old folder was not recreated");
}
