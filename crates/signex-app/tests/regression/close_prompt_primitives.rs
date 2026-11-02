//! #1843 — Close Project lists the project's dirty symbol and footprint
//! editors, as the quit prompt does, and Save All / Discard All act on them.

use signex_app::app::{
    Message, ProjectCloseChoice, ProjectMsg, Signex, SymbolDocument, TabInfo, TabKind,
};
use signex_types::project::LibraryEntryKind;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

use crate::primitive_dirty_quit::{
    app_with_open_project, edit_description_via_panel, open_footprint_editor, preview_edit_in,
    push_project,
};

/// A dirty symbol editor with a tab at `path`.
fn open_dirty_symbol(app: &mut Signex, path: &Path) {
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("Sym1"));
    app.document_state.symbol_editors.insert(
        path.to_path_buf(),
        SymbolDocument::new(path.to_path_buf(), file),
    );
    app.document_state.tabs.push(TabInfo {
        title: "Sym1".into(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(path.to_path_buf()),
    });
    app.document_state.mark_document_dirty(path);
}

fn symbol_path(dir: &Path) -> PathBuf {
    dir.join("symbols").join("s.snxsym")
}

#[test]
fn a_dirty_symbol_alone_makes_close_project_prompt() {
    let tmp = TempDir::new().unwrap();
    let symbol = symbol_path(tmp.path());
    let mut app = app_with_open_project(tmp.path());
    open_dirty_symbol(&mut app, &symbol);

    crate::close_support::close_first_project(&mut app);

    let modal = app
        .ui_state
        .project_close_confirm
        .as_ref()
        .expect("a dirty symbol editor must open the Close Project prompt");
    assert_eq!(modal.dirty_paths, vec![symbol]);
    assert_eq!(app.document_state.projects.len(), 1, "stays open for now");
}

#[test]
fn a_dirty_footprint_makes_close_project_prompt() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("footprints").join("d.snxfpt");
    let mut app = app_with_open_project(tmp.path());
    open_footprint_editor(&mut app, &path);
    edit_description_via_panel(&mut app);

    crate::close_support::close_first_project(&mut app);

    let modal = app.ui_state.project_close_confirm.as_ref().expect("prompt");
    assert_eq!(modal.dirty_paths, vec![path]);
}

#[test]
fn save_all_writes_the_symbol_and_closes_the_project() {
    let tmp = TempDir::new().unwrap();
    let symbol = symbol_path(tmp.path());
    std::fs::create_dir_all(symbol.parent().unwrap()).unwrap();
    let mut app = app_with_open_project(tmp.path());
    open_dirty_symbol(&mut app, &symbol);
    crate::close_support::close_first_project(&mut app);

    crate::settle_support::settle(
        &mut app,
        Message::Project(ProjectMsg::CloseConfirm(ProjectCloseChoice::SaveAll)),
    );

    assert!(symbol.exists(), "Save All must write the .snxsym");
    assert!(!app.document_state.is_document_dirty(&symbol));
    assert!(app.document_state.projects.is_empty(), "project closed");
}

#[test]
fn discard_all_clears_the_symbol_and_its_tab() {
    let tmp = TempDir::new().unwrap();
    let symbol = symbol_path(tmp.path());
    let mut app = app_with_open_project(tmp.path());
    open_dirty_symbol(&mut app, &symbol);
    crate::close_support::close_first_project(&mut app);

    let _ = app.update(Message::Project(ProjectMsg::CloseConfirm(
        ProjectCloseChoice::DiscardAll,
    )));

    assert!(!app.document_state.is_document_dirty(&symbol));
    assert!(!app.document_state.symbol_editors.contains_key(&symbol));
    assert!(app.document_state.tabs.iter().all(|t| t.path != symbol));
    assert!(!symbol.exists(), "nothing was written");
}

#[test]
fn a_symbol_and_a_row_are_listed_together() {
    let tmp = TempDir::new().unwrap();
    let symbol = symbol_path(tmp.path());
    let app = app_with_open_project(tmp.path());
    let mut app = preview_edit_in(app, tmp.path().join("lib.snxlib"));
    open_dirty_symbol(&mut app, &symbol);

    crate::close_support::close_first_project(&mut app);

    let modal = app.ui_state.project_close_confirm.as_ref().expect("prompt");
    assert_eq!(modal.dirty_rows.len(), 1);
    assert_eq!(modal.dirty_paths, vec![symbol]);
}

#[test]
fn an_outer_project_does_not_claim_the_drafts_of_a_nested_project() {
    let tmp = TempDir::new().unwrap();
    let mut app = app_with_open_project(tmp.path());
    let sub = tmp.path().join("sub");
    push_project(&mut app, &sub, "b");
    open_dirty_symbol(&mut app, &symbol_path(&sub));

    crate::close_support::close_first_project(&mut app);

    assert!(
        app.ui_state.project_close_confirm.is_none(),
        "B's symbol is not A's to save or discard",
    );
    assert!(app.document_state.is_document_dirty(&symbol_path(&sub)));
}

/// Re-list project `index`'s library entry as `kind`, at `path`.
fn relist_library(app: &mut Signex, index: usize, kind: LibraryEntryKind, path: PathBuf) {
    let entry = &mut app.document_state.projects[index].data.libraries[0];
    entry.kind = kind;
    entry.path = path;
}

#[test]
fn a_shared_librarys_draft_is_kept_on_close_project_and_not_listed() {
    // The shared library stays mounted when the project closes, so its draft
    // is not the project's to save or discard: it stays open and dirty.
    let tmp = TempDir::new().unwrap();
    let lib = tmp.path().join("shared").join("g.snxlib");
    let draft = lib.parent().unwrap().join("footprints").join("d.snxfpt");
    let mut app = app_with_open_project(tmp.path());
    relist_library(&mut app, 0, LibraryEntryKind::Shared, lib);
    open_footprint_editor(&mut app, &draft);
    edit_description_via_panel(&mut app);

    crate::close_support::close_first_project(&mut app);

    assert!(
        app.ui_state.project_close_confirm.is_none(),
        "a shared library's draft is not offered to Close Project",
    );
    assert!(app.document_state.projects.is_empty(), "the project closes");
    assert!(app.document_state.is_document_dirty(&draft));
    assert!(app.document_state.footprint_editors.contains_key(&draft));
    assert!(app.document_state.tabs.iter().any(|t| t.path == draft));
}

#[test]
fn a_library_listed_by_two_open_projects_keeps_its_drafts() {
    let tmp = TempDir::new().unwrap();
    let common = tmp.path().join("common").join("lib.snxlib");
    let mut app = app_with_open_project(&tmp.path().join("a"));
    push_project(&mut app, &tmp.path().join("b"), "b");
    for index in 0..2 {
        relist_library(
            &mut app,
            index,
            LibraryEntryKind::ProjectLocal,
            PathBuf::from("../common/lib.snxlib"),
        );
    }
    let draft = common.parent().unwrap().join("symbols").join("s.snxsym");
    open_dirty_symbol(&mut app, &draft);

    crate::close_support::close_first_project(&mut app);

    assert!(
        app.ui_state.project_close_confirm.is_none(),
        "the other open project lists the library too",
    );
    assert!(app.document_state.is_document_dirty(&draft));
}

/// A project nested inside another project's folder keeps its own drafts:
/// closing the outer project must not list (or save, or discard) them, even
/// when the outer project's library is the deepest library folder above them.
#[test]
fn an_outer_project_does_not_claim_a_draft_in_a_nested_projects_folder() {
    let tmp = TempDir::new().unwrap();
    let mut app = app_with_open_project(tmp.path());
    let sub = tmp.path().join("sub");
    push_project(&mut app, &sub, "b");
    relist_library(
        &mut app,
        1,
        LibraryEntryKind::ProjectLocal,
        PathBuf::from("libs/b.snxlib"),
    );
    let draft = sub.join("symbols").join("s.snxsym");
    open_dirty_symbol(&mut app, &draft);

    crate::close_support::close_first_project(&mut app);

    assert!(
        app.ui_state.project_close_confirm.is_none(),
        "B's folder is not A's to save or discard"
    );
    assert!(app.document_state.is_document_dirty(&draft));
}
