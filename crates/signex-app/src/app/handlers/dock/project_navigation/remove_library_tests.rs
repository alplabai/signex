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

    assert!(matches!(io, DeleteIo::DeletedLeaving(_)), "{io:?}");
    assert!(matches!(
        entry,
        crate::app::project_entry_disk::EntryPatch::Patched(_)
    ));
    assert!(!f.manifest.exists());
    let saved = signex_types::project::parse_project(&project_file).unwrap();
    assert!(saved.libraries.is_empty());
}
