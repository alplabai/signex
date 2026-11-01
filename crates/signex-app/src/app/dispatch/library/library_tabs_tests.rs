//! #1844 — Close Library closes the library's Library Browser and Component
//! Preview tabs, and a preview tab's title follows its row's part number.
//! Every step goes through `update`, the way the runtime drives it.

use super::*;
use crate::app::{TabInfo, TabKind};
use crate::library::messages::CloseLibraryChoice;

fn send(f: &mut Fixture, msg: LibraryMessage) {
    let task = f.app.update(Message::Library(msg));
    run_task(&mut f.app, task);
}

fn request_close(f: &mut Fixture) {
    let file = f.file.clone();
    send(f, LibraryMessage::CloseLibrary(file));
}

fn open_browser_tab(f: &mut Fixture) {
    send(f, LibraryMessage::OpenLibraryBrowser(f.file.clone()));
}

fn open_preview_tab(f: &mut Fixture, row: usize) {
    send(
        f,
        LibraryMessage::OpenComponentRow {
            library_path: f.file.clone(),
            table: TABLE.into(),
            row_id: f.rows[row],
        },
    );
}

/// A browser tab of a library other than the fixture's.
fn other_library_tab(f: &mut Fixture) -> PathBuf {
    let other = f.file.with_file_name("Other.snxlib");
    f.app.document_state.tabs.push(TabInfo {
        title: "Other".into(),
        path: other.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::LibraryBrowser(other.clone()),
    });
    other
}

/// The tabs that show the fixture's library.
fn library_tabs(f: &Fixture) -> Vec<String> {
    f.app
        .document_state
        .tabs
        .iter()
        .filter(|tab| match &tab.kind {
            TabKind::LibraryBrowser(path) => *path == f.file,
            TabKind::ComponentEditor(preview) => preview.library_path == f.file,
            _ => false,
        })
        .map(|tab| tab.path.display().to_string())
        .collect()
}

fn preview_title(f: &Fixture, row: usize) -> String {
    let path = f.address(row).synthetic_tab_path();
    f.app
        .document_state
        .tabs
        .iter()
        .find(|tab| tab.path == path)
        .map(|tab| tab.title.clone())
        .expect("the preview tab is open")
}

#[test]
fn close_library_closes_its_browser_and_preview_tabs() {
    let mut f = fixture();
    let other = other_library_tab(&mut f);
    open_browser_tab(&mut f);
    open_preview_tab(&mut f, 0);
    assert_eq!(library_tabs(&f).len(), 2, "browser and preview are open");

    request_close(&mut f);

    assert!(f.app.library.library_at(&f.file).is_none(), "unmounted");
    assert_eq!(
        library_tabs(&f),
        Vec::<String>::new(),
        "its tabs are closed"
    );
    assert!(
        f.app
            .document_state
            .tabs
            .iter()
            .any(|tab| tab.path == other),
        "another library's tab stays open"
    );
}

#[test]
fn a_close_that_waits_for_a_write_closes_the_tabs_when_it_completes() {
    let mut f = fixture();
    open_browser_tab(&mut f);
    open_preview_tab(&mut f, 1);
    let write = start_cell_edit(&mut f);

    request_close(&mut f);
    assert_eq!(
        library_tabs(&f).len(),
        2,
        "the library is still mounted while the close waits, so its tabs stay"
    );

    finish(&mut f.app, write);

    assert!(f.app.library.library_at(&f.file).is_none(), "unmounted");
    assert_eq!(library_tabs(&f), Vec::<String>::new());
}

#[test]
fn a_dirty_preview_tab_stays_until_the_close_prompt_is_answered() {
    let mut f = fixture();
    open_browser_tab(&mut f);
    open_preview_tab(&mut f, 0);
    let address = f.address(0);
    f.app.library.editors.get_mut(&address).unwrap().dirty = true;

    request_close(&mut f);
    assert!(
        f.app.library.close_library_confirm.is_some(),
        "the user is asked"
    );
    assert_eq!(
        library_tabs(&f).len(),
        2,
        "nothing is closed before the answer"
    );

    send(
        &mut f,
        LibraryMessage::CloseLibraryConfirm(CloseLibraryChoice::DiscardAll),
    );

    assert_eq!(library_tabs(&f), Vec::<String>::new());
}

#[test]
fn a_preview_tab_title_follows_the_part_number_set_in_the_browser() {
    let mut f = fixture();
    open_preview_tab(&mut f, 0);
    assert_eq!(preview_title(&f, 0), "R-1");

    let write = start_cell_edit(&mut f);
    finish(&mut f.app, write);

    assert_eq!(f.stored_pn(0), "CELL-EDIT", "the part number was saved");
    assert_eq!(preview_title(&f, 0), "CELL-EDIT");
}
