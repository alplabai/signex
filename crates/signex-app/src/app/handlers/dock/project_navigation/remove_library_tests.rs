//! #1847 — removing a library unmounts it, a delete takes only the library's
//! own files, and the dialog says how many placed parts use it.

use std::fs;
use std::path::PathBuf;

use uuid::Uuid;

use super::remove_library_usage::{LibraryUsage, usage_text};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::path_ops::settle;
use crate::app::{
    ContextMenuMsg, LibraryRowRef, Message, ProjectTreeAction, RemoveChoice, RemoveMsg, Signex,
};
use crate::library::state::{LibraryDisplaySettings, OpenLibrary};
use signex_types::project::{LibraryEntry, LibraryEntryKind};

struct Fixture {
    app: Signex,
    _tmp: tempfile::TempDir,
    dir: PathBuf,
    manifest: PathBuf,
    row: LibraryRowRef,
}

/// A project in a tempdir listing `libs/Power/Power.snxlib`, mounted, with
/// symbol files, a datasheet and an unrelated user file beside it.
fn fixture() -> Fixture {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project_dir = tmp.path().to_path_buf();
    let dir = project_dir.join("libs").join("Power");
    fs::create_dir_all(dir.join("symbols")).unwrap();
    fs::create_dir_all(dir.join("assets/datasheets")).unwrap();
    let manifest = dir.join("Power.snxlib");
    fs::write(&manifest, b"manifest").unwrap();
    fs::write(dir.join("symbols/a.snxsym"), b"sym").unwrap();
    fs::write(dir.join("assets/datasheets/d.pdf"), b"pdf").unwrap();
    fs::write(dir.join("my-notes.txt"), b"keep me").unwrap();
    fs::write(project_dir.join("ProjectA.snxprj"), b"{}").unwrap();

    let mut app = app_workspace(&project_dir.to_string_lossy(), &[]);
    app.document_state.projects[0]
        .data
        .libraries
        .push(LibraryEntry {
            path: PathBuf::from("libs/Power/Power.snxlib"),
            kind: LibraryEntryKind::ProjectLocal,
            library_id: None,
            unknown: Default::default(),
        });
    app.refresh_panel_ctx();
    app.library.open_libraries.push(OpenLibrary {
        root: manifest.clone(),
        display_name: "Power".into(),
        library_id: Uuid::now_v7(),
        tables: Default::default(),
        cached_components: Vec::new(),
        cached_symbols: Vec::new(),
        cached_footprints: Vec::new(),
        cached_sims: Vec::new(),
        cached_classes: Vec::new(),
        observed_stamp: None,
        dangling: Default::default(),
        pricing: Default::default(),
        own_edit_stamp: None,
        display: LibraryDisplaySettings::default(),
    });
    let row = LibraryRowRef {
        project_id: app.document_state.projects[0].id,
        root: manifest.clone(),
    };
    Fixture {
        app,
        _tmp: tmp,
        dir,
        manifest,
        row,
    }
}

fn remove(f: &mut Fixture, choice: RemoveChoice) {
    let _ = f
        .app
        .update(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
            ProjectTreeAction::RemoveLibrary(f.row.clone()),
        )));
    assert!(f.app.ui_state.remove_dialog.is_some(), "dialog opened");
    settle(&mut f.app, Message::Remove(RemoveMsg::Confirm(choice)));
}

#[test]
fn delete_takes_library_files_only_and_unmounts() {
    let mut f = fixture();

    remove(&mut f, RemoveChoice::DeleteFile);

    assert!(!f.manifest.exists());
    assert!(!f.dir.join("symbols").exists());
    assert!(!f.dir.join("assets").exists());
    assert_eq!(fs::read(f.dir.join("my-notes.txt")).unwrap(), b"keep me");
    assert!(f.app.library.library_at(&f.manifest).is_none());
    assert!(f.app.document_state.projects[0].data.libraries.is_empty());
}

#[test]
fn exclude_keeps_the_files_and_unmounts() {
    let mut f = fixture();

    remove(&mut f, RemoveChoice::ExcludeFromProject);

    assert!(f.manifest.exists());
    assert!(f.dir.join("symbols/a.snxsym").exists());
    assert!(f.app.library.library_at(&f.manifest).is_none());
}

#[test]
fn the_dialog_counts_parts_and_flags_unchecked_sheets() {
    let mut f = fixture();
    let _ = f
        .app
        .update(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
            ProjectTreeAction::RemoveLibrary(f.row.clone()),
        )));
    let dialog = f.app.ui_state.remove_dialog.clone().expect("dialog");

    let line = f.app.library_usage_line(&dialog).expect("library mounted");
    assert!(line.contains("No parts"), "{line}");

    let text = usage_text(&LibraryUsage {
        placed_parts: 3,
        unchecked_sheets: 2,
    });
    assert!(text.contains("3 parts"), "{text}");
    assert!(text.contains("2 sheet(s) that are not open"), "{text}");
}

/// A second project lists the same library, so the first project removing it
/// leaves it mounted.
fn list_in_second_project(f: &mut Fixture) {
    let mut other = app_workspace(&f.dir.to_string_lossy(), &[])
        .document_state
        .projects
        .remove(0);
    other.id = f.app.document_state.mint_project_id();
    other.path = f.dir.join("ProjectB.snxprj");
    other.data.libraries.push(LibraryEntry {
        path: f.manifest.clone(),
        kind: LibraryEntryKind::Shared,
        library_id: None,
        unknown: Default::default(),
    });
    f.app.document_state.projects.push(other);
}

#[test]
fn another_projects_library_stays_mounted_after_exclude() {
    let mut f = fixture();
    list_in_second_project(&mut f);

    remove(&mut f, RemoveChoice::ExcludeFromProject);

    assert!(f.app.library.library_at(&f.manifest).is_some());
}

#[test]
fn exclude_with_a_dirty_editor_asks_instead_of_dropping_it() {
    let mut f = fixture();
    let row = crate::library::component_preview::test_row();
    let address = crate::library::state::EditorAddress::new(
        f.manifest.clone(),
        "components".into(),
        signex_library::RowId::from_uuid(row.row_id),
    );
    let mut preview = crate::library::state::ComponentPreviewState::from_row(
        f.manifest.clone(),
        "components".into(),
        row,
    );
    preview.dirty = true;
    f.app.library.editors.insert(address, preview);

    remove(&mut f, RemoveChoice::ExcludeFromProject);

    assert!(f.app.library.close_library_confirm.is_some());
    assert!(f.app.library.library_at(&f.manifest).is_some());
}

#[test]
fn a_sibling_library_in_the_folder_keeps_its_primitives() {
    let mut f = fixture();
    fs::write(f.dir.join("Other.snxlib"), b"sibling").unwrap();

    remove(&mut f, RemoveChoice::DeleteFile);

    assert!(!f.manifest.exists());
    assert!(f.dir.join("Other.snxlib").exists());
    assert!(f.dir.join("symbols/a.snxsym").exists());
}

#[test]
fn the_libraries_lock_files_go_with_it() {
    let mut f = fixture();
    fs::write(f.dir.join(".Power.snxlib.signex-writelock"), b"").unwrap();
    fs::write(f.dir.join(".Power.snxlib.signex-writelock.queue"), b"").unwrap();

    remove(&mut f, RemoveChoice::DeleteFile);

    assert!(!f.dir.join(".Power.snxlib.signex-writelock").exists());
    assert!(!f.dir.join(".Power.snxlib.signex-writelock.queue").exists());
}

/// A saved project listing `libs/Power/Power.snxlib`, plus a delete job for it.
fn job_with_saved_project(f: &Fixture) -> (crate::app::path_op_jobs::DeleteJob, PathBuf) {
    let project_dir = f.dir.parent().unwrap().parent().unwrap().to_path_buf();
    let project_file = project_dir.join("ProjectA.snxprj");
    signex_types::project::write_project(&project_file, &f.app.document_state.projects[0].data)
        .unwrap();
    let job = crate::app::path_op_jobs::DeleteJob {
        target: f.manifest.clone(),
        project_root: Some(project_dir),
        shared_libraries: Vec::new(),
        saves: f.app.document_state.document_saves.barrier(),
        project: Some((
            project_file.clone(),
            crate::app::state::scope::CaseFold::from_probe(false),
        )),
    };
    (job, project_file)
}

#[test]
fn a_library_delete_drops_the_saved_entry_and_reports_leftovers() {
    use crate::app::path_ops::DeleteIo;
    let f = fixture();
    let (job, project_file) = job_with_saved_project(&f);

    let (io, entry) = job.run();

    assert!(
        matches!(&io, DeleteIo::DeletedLibrary { left, .. } if !left.is_empty()),
        "{io:?}"
    );
    assert!(matches!(
        entry,
        crate::app::project_entry_disk::EntryPatch::Patched(_)
    ));
    assert!(!f.manifest.exists());
    let saved = signex_types::project::parse_project(&project_file).unwrap();
    assert!(saved.libraries.is_empty());
}

/// A clean footprint editor tab on `path`.
fn footprint_tab(f: &mut Fixture, path: &std::path::Path) {
    let file =
        signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("fp"));
    f.app.document_state.footprint_editors.insert(
        path.to_path_buf(),
        crate::app::FootprintDocument::new(path.to_path_buf(), file),
    );
    f.app.document_state.tabs.push(crate::app::TabInfo {
        title: "fp".into(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: None,
        kind: crate::app::TabKind::FootprintEditor(path.to_path_buf()),
    });
}

#[test]
fn delete_closes_the_tabs_of_the_primitives_it_removed() {
    let mut f = fixture();
    let removed = f.dir.join("symbols/a.snxsym");
    let kept = f.dir.join("my-notes.txt");
    footprint_tab(&mut f, &removed);
    footprint_tab(&mut f, &kept);

    remove(&mut f, RemoveChoice::DeleteFile);

    assert!(
        !f.app
            .document_state
            .footprint_editors
            .contains_key(&removed)
    );
    assert!(f.app.document_state.footprint_editors.contains_key(&kept));
    let tabs: Vec<_> = f.app.document_state.tabs.iter().map(|t| &t.path).collect();
    assert_eq!(tabs, [&kept]);
}

#[test]
fn delete_is_refused_while_a_primitive_draft_is_unsaved() {
    let mut f = fixture();
    let draft = f.dir.join("symbols/a.snxsym");
    footprint_tab(&mut f, &draft);
    f.app.document_state.mark_document_dirty(&draft);

    remove(&mut f, RemoveChoice::DeleteFile);

    assert!(f.manifest.exists());
    assert!(draft.exists());
    assert!(f.app.document_state.error_notice.is_some());
    assert!(f.app.document_state.is_document_dirty(&draft));
}

#[test]
fn delete_is_refused_while_a_library_write_is_queued() {
    let mut f = fixture();
    let _turn = f.app.library.writes.turn(&f.manifest);

    remove(&mut f, RemoveChoice::DeleteFile);

    assert!(f.manifest.exists());
    assert!(f.app.document_state.error_notice.is_some());
}

#[test]
fn delete_is_refused_while_the_library_is_still_mounting() {
    let mut f = fixture();
    f.app.library.pending_mounts.insert(
        f.manifest.clone(),
        crate::library::mount::MountIntent::Silent,
    );

    remove(&mut f, RemoveChoice::DeleteFile);

    assert!(f.manifest.exists());
    assert!(f.app.document_state.error_notice.is_some());
}

#[test]
fn delete_unmounts_even_when_another_project_lists_it() {
    let mut f = fixture();
    list_in_second_project(&mut f);

    remove(&mut f, RemoveChoice::DeleteFile);

    assert!(!f.manifest.exists());
    assert!(f.app.library.library_at(&f.manifest).is_none());
}

#[test]
fn a_project_model_folder_beside_the_library_survives_delete() {
    let mut f = fixture();
    fs::create_dir_all(f.dir.join("models")).unwrap();
    fs::write(f.dir.join("models/board.step"), b"pcb model").unwrap();

    remove(&mut f, RemoveChoice::DeleteFile);

    assert!(!f.manifest.exists());
    assert_eq!(
        fs::read(f.dir.join("models/board.step")).unwrap(),
        b"pcb model"
    );
}

#[test]
fn a_primitive_under_the_library_cannot_be_saved_while_its_delete_runs() {
    let mut f = fixture();
    let draft = f.dir.join("symbols/a.snxsym");
    footprint_tab(&mut f, &draft);
    let _ = f
        .app
        .update(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
            ProjectTreeAction::RemoveLibrary(f.row.clone()),
        )));
    // The delete worker is started but not run yet.
    let _worker = f.app.update(Message::Remove(RemoveMsg::Confirm(
        RemoveChoice::DeleteFile,
    )));
    assert!(
        f.app.refuse_while_path_op(&draft, "saved").is_err(),
        "a save during the delete worker is either deleted after it reports saved, or recreates an orphan"
    );
}

#[test]
fn delete_with_a_dirty_component_editor_is_refused() {
    let mut f = fixture();
    let row = crate::library::component_preview::test_row();
    let address = crate::library::state::EditorAddress::new(
        f.manifest.clone(),
        "components".into(),
        signex_library::RowId::from_uuid(row.row_id),
    );
    let mut preview = crate::library::state::ComponentPreviewState::from_row(
        f.manifest.clone(),
        "components".into(),
        row,
    );
    preview.dirty = true;
    f.app.library.editors.insert(address, preview);

    remove(&mut f, RemoveChoice::DeleteFile);

    assert!(
        f.manifest.exists(),
        "unsaved row edits dropped without asking"
    );
    assert!(f.app.document_state.error_notice.is_some());
}

#[test]
fn a_folder_with_a_project_file_keeps_its_primitives() {
    let mut f = fixture();
    fs::write(f.dir.join("Board.SNXPRJ"), b"{}").unwrap();

    remove(&mut f, RemoveChoice::DeleteFile);

    assert!(!f.manifest.exists());
    assert!(f.dir.join("symbols/a.snxsym").exists());
}

/// Guard for the report path: the primitive editor dispatch refuses edits
/// during a library delete (#1939), but the Component Preview and undo/redo
/// can still dirty an editor, so the test dirties it directly.
#[test]
fn an_editor_dirtied_while_the_delete_ran_is_reported_not_dropped_silently() {
    let _ = crate::diagnostics::init_logging();
    let mut f = fixture();
    let delete = start_delete(&mut f);
    // The editor goes dirty after Confirm checked for dirty ones, while the
    // delete worker runs.
    dirty_preview(&mut f);
    crate::app::path_ops::settle_task(&mut f.app, delete);

    assert!(!f.manifest.exists(), "the delete went through");
    assert_eq!(
        lost_edit_lines(&f),
        1,
        "the discarded edits are named in Messages, with this library's path"
    );
}

#[test]
fn a_delete_that_found_nothing_on_disk_still_says_the_editor_was_dropped() {
    let _ = crate::diagnostics::init_logging();
    let mut f = fixture();
    let delete = start_delete(&mut f);
    // The file is gone before the worker looks: this app deletes nothing.
    fs::remove_file(&f.manifest).unwrap();
    dirty_preview(&mut f);
    crate::app::path_ops::settle_task(&mut f.app, delete);

    assert_eq!(lost_edit_lines(&f), 1, "the dropped edits are named");
    let library = f.manifest.display().to_string();
    assert!(
        crate::diagnostics::recent_entries().iter().any(|e| {
            e.message.contains(&library) && e.message.contains("already gone from disk")
        }),
        "the line says the file was already gone, not that it was deleted"
    );
}

/// Open the Remove dialog for `f`'s library and confirm Delete; the worker
/// task is returned unrun.
fn start_delete(f: &mut Fixture) -> iced::Task<Message> {
    let _ = f
        .app
        .update(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
            ProjectTreeAction::RemoveLibrary(f.row.clone()),
        )));
    f.app.update(Message::Remove(RemoveMsg::Confirm(
        RemoveChoice::DeleteFile,
    )))
}

/// A Component Preview of `f`'s library, made dirty.
fn dirty_preview(f: &mut Fixture) {
    let row = crate::library::component_preview::test_row();
    let address = crate::library::state::EditorAddress::new(
        f.manifest.clone(),
        "components".into(),
        signex_library::RowId::from_uuid(row.row_id),
    );
    let mut preview = crate::library::state::ComponentPreviewState::from_row(
        f.manifest.clone(),
        "components".into(),
        row,
    );
    preview.dirty = true;
    f.app.library.editors.insert(address, preview);
}

/// Messages that name the library at `f.manifest` and say edits were lost.
fn lost_edit_lines(f: &Fixture) -> usize {
    let library = f.manifest.display().to_string();
    crate::diagnostics::recent_entries()
        .iter()
        .filter(|e| e.message.contains(&library) && e.message.contains("unsaved edits"))
        .count()
}
