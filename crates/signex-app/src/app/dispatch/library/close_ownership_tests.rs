//! #1900 — edge cases of which unsaved symbol / footprint draft the Close
//! Library prompt asks about. None may let a library close over a draft.

use super::close_primitives_tests::{
    mount_library_at, open_dirty_symbol, request_close, send, symbol_in_library,
};
use super::deferred_close_tests::dirty_symbol_in_library;
use super::*;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::library::messages::CloseLibraryChoice;
use signex_types::project::{LibraryEntry, LibraryEntryKind};

fn asked_about(f: &Fixture) -> Vec<PathBuf> {
    f.app
        .library
        .close_library_confirm
        .as_ref()
        .expect("the prompt is up")
        .dirty_paths
        .clone()
}

/// An open project in the library folder that lists `vendor/V.snxlib`
/// without it being mounted.
fn list_unmounted_vendor_library(f: &mut Fixture) {
    let dir = f.file.parent().unwrap().to_string_lossy().into_owned();
    let mut listed = app_workspace(&dir, &[]);
    let mut project = listed.document_state.projects.remove(0);
    project.id = f.app.document_state.mint_project_id();
    project.data.libraries.push(LibraryEntry {
        path: PathBuf::from("vendor").join("V.snxlib"),
        kind: LibraryEntryKind::ProjectLocal,
        library_id: None,
        unknown: Default::default(),
    });
    f.app.document_state.projects.push(project);
}

#[test]
fn a_library_listed_by_a_project_but_not_mounted_does_not_take_a_draft() {
    let mut f = fixture();
    list_unmounted_vendor_library(&mut f);
    let draft = f
        .file
        .parent()
        .unwrap()
        .join("vendor")
        .join("symbols")
        .join("x.snxsym");
    open_dirty_symbol(&mut f, &draft);

    request_close(&mut f);

    assert_eq!(
        asked_about(&f),
        vec![draft],
        "nothing mounted owns it better, so closing asks"
    );
    assert!(f.app.library.library_at(&f.file).is_some(), "still mounted");
}

#[test]
fn closing_either_of_two_libraries_in_one_folder_asks_about_their_draft() {
    let mut f = fixture();
    let sibling = f.file.parent().unwrap().join("Other.snxlib");
    mount_library_at(&mut f, sibling.clone());
    let draft = symbol_in_library(&f, "s.snxsym");
    open_dirty_symbol(&mut f, &draft);

    request_close(&mut f);
    assert_eq!(asked_about(&f), vec![draft.clone()], "the first library");
    send(
        &mut f,
        LibraryMessage::CloseLibraryConfirm(CloseLibraryChoice::Cancel),
    );

    send(&mut f, LibraryMessage::CloseLibrary(sibling));
    assert_eq!(asked_about(&f), vec![draft], "and the second");
}

#[test]
fn a_draft_with_its_save_in_flight_still_raises_the_close_prompt() {
    let mut f = fixture();
    let draft = dirty_symbol_in_library(&mut f);
    let _save = f.app.save_primitive_tab_at(&draft).expect("save starts");
    assert!(
        !f.app.document_state.symbol_editors[&draft].is_dirty(),
        "the snapshot reset the editor flag"
    );

    request_close(&mut f);

    assert_eq!(asked_about(&f), vec![draft]);
    assert!(f.app.library.library_at(&f.file).is_some(), "still mounted");
}

#[test]
fn a_draft_edited_while_a_close_waits_holds_the_close_and_is_listed() {
    let mut f = fixture();
    let write = start_cell_edit(&mut f);
    request_close(&mut f);
    assert!(f.app.library.is_closing(&f.file), "the close waits");

    let draft = symbol_in_library(&f, "late.snxsym");
    open_dirty_symbol(&mut f, &draft);
    finish(&mut f.app, write);

    assert!(f.app.library.library_at(&f.file).is_some(), "close held");
    assert!(f.app.document_state.is_document_dirty(&draft));
    assert_eq!(asked_about(&f), vec![draft]);
}

#[test]
fn a_rename_during_the_prompt_moves_the_listed_drafts_of_a_folder_layout_library() {
    let mut f = fixture();
    let old = f.file.clone();
    let inside = old.join("symbols").join("s.snxsym");
    open_dirty_symbol(&mut f, &inside);
    request_close(&mut f);
    assert_eq!(asked_about(&f), vec![inside.clone()]);

    let new = old.with_file_name("Renamed.snxlib");
    f.app
        .library
        .relocate_library(&old, &new)
        .expect("relocate");

    let confirm = f.app.library.close_library_confirm.as_ref().unwrap();
    assert_eq!(confirm.library_path, new);
    assert_eq!(
        confirm.dirty_paths,
        vec![new.join("symbols").join("s.snxsym")]
    );
}
