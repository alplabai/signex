//! #859 — a rename or delete runs on a worker, so for a moment the file is
//! being moved while the app is still live. Nothing may write to, reopen or
//! re-move it in that window, edits made meanwhile must survive the re-key,
//! and a project or tab that closed meanwhile must not leave a key on a path
//! that no longer exists.

use signex_app::app::path_ops::{FileRenameDone, RenameIoError};
use signex_app::app::{
    FileMsg, LoadedProject, Message, ProjectId, RemoveChoice, RemoveDialogState, RemoveMsg,
    RenameDialogState, RenameMsg, Signex, TabInfo, TabKind,
};
use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::SchematicSheet;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

struct Fixture {
    app: Signex,
    tmp: TempDir,
    project: ProjectId,
    old: PathBuf,
    new: PathBuf,
}

fn sheet_entry(filename: &str) -> SheetEntry {
    SheetEntry {
        name: filename.to_string(),
        filename: filename.to_string(),
        symbols_count: 0,
        wires_count: 0,
        labels_count: 0,
        unknown: Default::default(),
    }
}

/// A loaded project listing `Power.snxsch`, which is open (and optionally
/// dirty) as the active tab and exists on disk.
fn fixture(dirty: bool) -> Fixture {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let dir = tmp.path().to_path_buf();
    let old = dir.join("Power.snxsch");
    let new = dir.join("PSU.snxsch");
    std::fs::write(&old, b"x").expect("write sheet");
    let (mut app, _task) = Signex::new();
    let project = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id: project,
        path: dir.join("Proj.snxprj"),
        data: ProjectData {
            name: "Proj".to_string(),
            dir: dir.to_string_lossy().into_owned(),
            schematic_root: Some("Power.snxsch".to_string()),
            pcb_file: None,
            sheets: vec![sheet_entry("Power.snxsch")],
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            rules: Default::default(),
            unknown: Default::default(),
        },
        pending_libraries: HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
    });
    app.document_state.active_project = Some(project);
    let mut engine = signex_engine::Engine::new(SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        ..Default::default()
    })
    .expect("engine");
    engine.set_path(Some(old.clone()));
    app.document_state.engines.insert(old.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "Power".to_string(),
        path: old.clone(),
        cached_document: None,
        dirty,
        project_id: Some(project),
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(old.clone());
    if dirty {
        app.document_state.dirty_paths.insert(old.clone());
    }
    Fixture {
        app,
        tmp,
        project,
        old,
        new,
    }
}

fn run(task: iced::Task<Message>) -> Vec<Message> {
    use futures::StreamExt as _;
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return Vec::new();
    };
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    runtime.block_on(async {
        let mut out = Vec::new();
        while let Some(action) = stream.next().await {
            if let iced_runtime::Action::Output(message) = action {
                out.push(message);
            }
        }
        out
    })
}

/// Feed the worker's completion messages back, without the follow-up tasks
/// (git commits) they may queue.
fn complete(app: &mut Signex, task: iced::Task<Message>) {
    for message in run(task) {
        let _follow_up = app.update(message);
    }
}

fn open_rename_dialog(app: &mut Signex, target: &Path, buffer: &str) {
    app.ui_state.rename_dialog = Some(RenameDialogState {
        project_id: None,
        target_path: target.to_path_buf(),
        tree_path: vec![0],
        buffer: buffer.to_string(),
        error: None,
        is_project_rename: false,
    });
}

/// Submit the rename and return the not-yet-run worker task.
fn start_rename(app: &mut Signex, target: &Path, buffer: &str) -> iced::Task<Message> {
    open_rename_dialog(app, target, buffer);
    app.update(Message::Rename(RenameMsg::Submit))
}

fn start_delete(app: &mut Signex, target: &Path) -> iced::Task<Message> {
    app.ui_state.remove_dialog = Some(RemoveDialogState {
        project_id: None,
        target_path: target.to_path_buf(),
        tree_path: vec![0],
        display_name: target.file_name().unwrap().to_string_lossy().into_owned(),
        is_outside_project: false,
        unsaved_prompt: false,
    });
    app.update(Message::Remove(RemoveMsg::Confirm(
        RemoveChoice::DeleteFile,
    )))
}

fn logged(fragment: &str) -> bool {
    signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains(fragment))
}

fn edit(app: &mut Signex, path: &Path, paper: &str) {
    app.document_state
        .engines
        .get_mut(path)
        .expect("engine")
        .execute(signex_engine::Command::SetPaperSize {
            paper_size: paper.to_string(),
        })
        .expect("edit");
}

#[test]
fn the_rename_does_not_touch_disk_or_memory_until_the_worker_reports() {
    let mut f = fixture(false);
    let task = start_rename(&mut f.app, &f.old.clone(), "PSU.snxsch");

    assert!(
        f.old.exists() && !f.new.exists(),
        "update must not move the file"
    );
    assert!(f.app.document_state.engines.contains_key(&f.old));
    assert_eq!(f.app.document_state.tabs[0].path, f.old);

    complete(&mut f.app, task);

    assert!(!f.old.exists() && f.new.exists());
    assert!(f.app.document_state.engines.contains_key(&f.new));
    assert!(!f.app.document_state.engines.contains_key(&f.old));
    assert_eq!(f.app.document_state.tabs[0].path, f.new);
    assert!(f.app.ui_state.rename_dialog.is_none());
    assert!(
        f.app
            .document_state
            .pending_path_ops
            .blocking_write(&f.new)
            .is_none(),
        "the registration is gone once it completes"
    );
}

#[test]
fn saving_the_sheet_while_it_is_being_renamed_is_refused_and_does_not_recreate_it() {
    let mut f = fixture(true);
    let task = start_rename(&mut f.app, &f.old.clone(), "PSU.snxsch");

    let save = f.app.update(Message::File(FileMsg::Save));
    let written = run(save);

    assert!(
        !written
            .iter()
            .any(|m| matches!(m, Message::File(FileMsg::DocumentSaved(_)))),
        "no write was started"
    );
    assert!(logged("A rename of Power.snxsch is still running"));

    complete(&mut f.app, task);
    assert!(f.new.exists());
    assert!(
        !f.old.exists(),
        "the refused save must not recreate the old name"
    );
    assert!(
        f.app.document_state.dirty_paths.contains(&f.new),
        "the unsaved edit stays dirty under the new name"
    );
}

/// Close Project's Save All reports the sheet that is being moved as that
/// file's failure and does not write it.
#[test]
fn save_all_does_not_write_a_sheet_that_is_being_renamed() {
    let mut f = fixture(true);
    crate::close_support::close_first_project(&mut f.app);
    assert!(
        f.app.ui_state.project_close_confirm.is_some(),
        "the dirty sheet raises the prompt"
    );
    let task = start_rename(&mut f.app, &f.old.clone(), "PSU.snxsch");

    let _save_all = f
        .app
        .update(Message::Project(signex_app::app::ProjectMsg::CloseConfirm(
            signex_app::app::ProjectCloseChoice::SaveAll,
        )));

    assert_eq!(
        std::fs::read(&f.old).unwrap(),
        b"x",
        "Save All must not write the sheet that is being moved"
    );
    assert!(logged("A rename of Power.snxsch is still running"));
    complete(&mut f.app, task);
    assert!(f.new.exists() && !f.old.exists());
}

#[test]
fn an_edit_made_while_the_rename_runs_travels_with_the_document() {
    let mut f = fixture(false);
    let task = start_rename(&mut f.app, &f.old.clone(), "PSU.snxsch");

    edit(&mut f.app, &f.old.clone(), "A3");
    f.app.document_state.dirty_paths.insert(f.old.clone());
    f.app.document_state.tabs[0].dirty = true;
    complete(&mut f.app, task);

    let engine = f.app.document_state.engines.get(&f.new).expect("re-keyed");
    assert_eq!(engine.document().paper_size, "A3", "the edit was kept");
    assert!(f.app.document_state.dirty_paths.contains(&f.new));
    assert!(!f.app.document_state.dirty_paths.contains(&f.old));
}

#[test]
fn a_second_rename_of_the_same_file_while_one_runs_is_refused_in_the_dialog() {
    let mut f = fixture(false);
    let first = start_rename(&mut f.app, &f.old.clone(), "PSU.snxsch");

    let second = start_rename(&mut f.app, &f.old.clone(), "Supply.snxsch");

    assert!(run(second).is_empty(), "no second worker started");
    let error = f
        .app
        .ui_state
        .rename_dialog
        .as_ref()
        .and_then(|d| d.error.clone());
    assert!(
        error
            .as_deref()
            .is_some_and(|e| e.starts_with("A rename of Power.snxsch is still running")),
        "{error:?}"
    );
    complete(&mut f.app, first);
    assert!(f.new.exists() && !f.tmp.path().join("Supply.snxsch").exists());
}

#[test]
fn a_rename_onto_an_existing_file_leaves_everything_unchanged() {
    let mut f = fixture(true);
    std::fs::write(&f.new, b"other").expect("write target");
    let task = start_rename(&mut f.app, &f.old.clone(), "PSU.snxsch");
    complete(&mut f.app, task);

    assert_eq!(
        std::fs::read(&f.new).unwrap(),
        b"other",
        "target not replaced"
    );
    assert!(f.old.exists());
    assert!(f.app.document_state.engines.contains_key(&f.old));
    assert!(f.app.document_state.dirty_paths.contains(&f.old));
    assert_eq!(f.app.document_state.tabs[0].path, f.old);
    let error = f
        .app
        .ui_state
        .rename_dialog
        .as_ref()
        .and_then(|d| d.error.clone());
    assert_eq!(
        error.as_deref(),
        Some("A file with that name already exists.")
    );
    assert!(logged("A file with that name already exists"));
    assert!(
        f.app.document_state.projects[0].data.sheets[0].filename == "Power.snxsch",
        "no half re-key of the project"
    );
}

#[test]
fn a_rename_the_os_refuses_leaves_everything_unchanged_and_says_why() {
    let mut f = fixture(true);
    let task = start_rename(&mut f.app, &f.old.clone(), "PSU.snxsch");
    std::fs::remove_file(&f.old).expect("source vanishes before the worker runs");
    complete(&mut f.app, task);

    assert!(f.app.document_state.engines.contains_key(&f.old));
    assert_eq!(f.app.document_state.tabs[0].path, f.old);
    let error = f
        .app
        .ui_state
        .rename_dialog
        .as_ref()
        .and_then(|d| d.error.clone());
    assert!(
        error
            .as_deref()
            .is_some_and(|e| e.starts_with("Rename failed: "))
    );
    assert!(logged("Rename failed: "));
}

#[test]
fn closing_the_tab_while_the_rename_runs_leaves_no_key_on_the_old_path() {
    let mut f = fixture(false);
    let task = start_rename(&mut f.app, &f.old.clone(), "PSU.snxsch");

    f.app.document_state.tabs.clear();
    f.app.document_state.engines.clear();
    f.app.document_state.active_path = None;
    complete(&mut f.app, task);

    assert!(f.new.exists());
    assert!(f.app.document_state.engines.is_empty());
    assert!(f.app.document_state.tabs.is_empty());
    assert_eq!(
        f.app.document_state.projects[0].data.sheets[0].filename, "PSU.snxsch",
        "the project still follows the file"
    );
}

#[test]
fn closing_the_project_while_the_rename_runs_warns_and_still_rekeys_open_documents() {
    let mut f = fixture(true);
    let task = start_rename(&mut f.app, &f.old.clone(), "PSU.snxsch");

    let project = f.project;
    f.app.document_state.projects.retain(|p| p.id != project);
    complete(&mut f.app, task);

    assert!(f.new.exists());
    assert!(f.app.document_state.engines.contains_key(&f.new));
    assert!(!f.app.document_state.engines.contains_key(&f.old));
    assert!(f.app.document_state.dirty_paths.contains(&f.new));
    assert!(logged(
        "Its project was closed while Power.snxsch was renamed to PSU.snxsch"
    ));
}

#[test]
fn deleting_a_dirty_sheet_discards_it_after_the_worker_and_says_so() {
    let mut f = fixture(true);
    edit(&mut f.app, &f.old.clone(), "A3");
    let task = start_delete(&mut f.app, &f.old.clone());

    assert!(f.old.exists(), "update must not delete");
    complete(&mut f.app, task);

    assert!(!f.old.exists());
    assert!(f.app.document_state.engines.is_empty());
    assert!(!f.app.document_state.dirty_paths.contains(&f.old));
    assert!(logged("discarded 1 unsaved document(s)"));
    assert!(
        f.app
            .document_state
            .pending_path_ops
            .blocking_write(&f.old)
            .is_none()
    );
}

#[test]
fn saving_or_renaming_a_file_that_is_being_deleted_is_refused() {
    let mut f = fixture(true);
    let other = f.tmp.path().join("Other.snxsch");
    std::fs::write(&other, b"x").unwrap();
    let delete = start_delete(&mut f.app, &f.old.clone());

    let rename = start_rename(&mut f.app, &f.old.clone(), "PSU.snxsch");
    assert!(run(rename).is_empty());
    let error = f
        .app
        .ui_state
        .rename_dialog
        .as_ref()
        .and_then(|d| d.error.clone());
    assert!(error.is_some_and(|e| e.starts_with("A delete of Power.snxsch is still running")));

    complete(&mut f.app, delete);
    assert!(!f.old.exists() && !f.new.exists());
    assert!(other.exists(), "an unrelated file is untouched");
}

/// A read-only parent folder: `remove_file` fails with a permission error.
#[cfg(unix)]
#[test]
fn a_delete_the_os_refuses_keeps_the_file_and_reports_it() {
    use std::os::unix::fs::PermissionsExt;
    let mut f = fixture(false);
    let dir = f.tmp.path().to_path_buf();
    let task = start_delete(&mut f.app, &f.old.clone());
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    complete(&mut f.app, task);
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();

    assert!(f.old.exists(), "a refused delete keeps the file");
    assert!(f.app.document_state.error_notice.is_some());
    assert!(logged("Failed to delete project file"));
}

#[test]
fn a_duplicate_completion_finds_nothing_to_rekey_and_changes_nothing() {
    let mut f = fixture(false);
    let task = start_rename(&mut f.app, &f.old.clone(), "PSU.snxsch");
    let messages = run(task);
    // The completion arrives twice (or after state was reset): the second
    // copy finds nothing to re-key and must not panic or undo the first.
    let mut twice = messages.clone();
    twice.extend(messages);
    for message in twice {
        let _follow_up = f.app.update(message);
    }
    assert!(f.app.document_state.engines.contains_key(&f.new));
    assert_eq!(f.app.document_state.tabs[0].path, f.new);
}

#[test]
fn a_failed_completion_message_for_a_closed_dialog_raises_the_card() {
    let mut f = fixture(false);
    let task = start_rename(&mut f.app, &f.old.clone(), "PSU.snxsch");
    let mut messages = run(task);
    f.app.ui_state.rename_dialog = None;
    if let Some(Message::Rename(RenameMsg::FileRenamed(done))) = messages.first_mut() {
        let FileRenameDone { result, .. } = done.as_mut();
        *result = Err(RenameIoError::Os("denied".to_string()));
    }
    for message in messages {
        let _follow_up = f.app.update(message);
    }
    assert!(f.app.document_state.error_notice.is_some());
}

/// The Exclude prompt's Save (#738) goes through the same refusal: a file a
/// rename holds is not written, the prompt stays up, and nothing is removed.
#[test]
fn the_exclude_prompt_save_is_refused_while_a_rename_holds_the_file() {
    let mut f = fixture(true);
    f.app.ui_state.remove_dialog = Some(RemoveDialogState {
        project_id: None,
        target_path: f.old.clone(),
        tree_path: vec![0],
        display_name: "Power.snxsch".to_string(),
        is_outside_project: false,
        unsaved_prompt: false,
    });
    let _prompt = f.app.update(Message::Remove(RemoveMsg::Confirm(
        RemoveChoice::ExcludeFromProject,
    )));
    assert!(
        f.app
            .ui_state
            .remove_dialog
            .as_ref()
            .unwrap()
            .unsaved_prompt
    );
    let task = start_rename(&mut f.app, &f.old.clone(), "PSU.snxsch");

    let _save = f.app.update(Message::Remove(RemoveMsg::ExcludeDirty(
        signex_app::app::ExcludeDirtyChoice::Save,
    )));

    assert_eq!(std::fs::read(&f.old).unwrap(), b"x", "nothing was written");
    assert!(
        f.app.ui_state.remove_dialog.is_some(),
        "the prompt stays up for a retry"
    );
    assert_eq!(f.app.document_state.projects[0].data.sheets.len(), 1);
    complete(&mut f.app, task);
}
