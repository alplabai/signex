//! m136 Close with drafts, m137 Delete row and m138 Recovery.

use std::path::PathBuf;

use signex_app::app::{Message, Signex, SymbolDocument, TabInfo, TabKind};
use signex_app::dock::DockMessage;
use signex_app::library::messages::{
    CloseLibraryChoice, LibraryMessage, PrimitiveEdit, SymbolEditorMsg,
};
use signex_app::library::recovery::{
    BrokenBindingChoice, GitMissingChoice, LibraryMissingChoice, RecoveryDialog,
};
use signex_app::library::state::PrimitivePickerTarget;
use signex_app::panels::PanelMsg;
use signex_library::{PrimitiveKind, RowId, Symbol, SymbolFile};
use signex_types::project::{LibraryEntry, LibraryEntryKind, parse_project};
use tempfile::TempDir;
use uuid::Uuid;

use super::support::{
    LibRig, TABLE, files_under, fresh_app, library_on_disk, library_rig, load_project,
    project_data, read_only_rig,
};
use crate::settle_support::settle;

// ---- m136 ----

/// A saved one-pin symbol inside the library, open with an unsaved pin name.
fn dirty_symbol(r: &mut LibRig) -> PathBuf {
    let dir = r.root.parent().expect("folder").join("symbols");
    std::fs::create_dir_all(&dir).expect("symbols folder");
    let path = dir.join("draft.snxsym");
    let file = SymbolFile::from_symbol(Symbol::empty("DRAFT"));
    std::fs::write(&path, file.to_toml_string().expect("encode")).expect("write symbol");
    r.app
        .document_state
        .symbol_editors
        .insert(path.clone(), SymbolDocument::new(path.clone(), file));
    r.app.document_state.tabs.push(TabInfo {
        title: "DRAFT".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::SymbolEditor(path.clone()),
    });
    r.app.document_state.active_tab = r.app.document_state.tabs.len() - 1;
    r.send(LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: PrimitiveEdit::Symbol(SymbolEditorMsg::AddPin { x: 0.0, y: 0.0 }),
    });
    let _ = r.app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::SymEditorSetPinName {
            pin_idx: 0,
            value: "EDITED".to_string(),
        },
    )));
    assert!(r.app.document_state.is_document_dirty(&path));
    path
}

fn saved_pin_names(path: &std::path::Path) -> Vec<String> {
    let bytes = std::fs::read(path).expect("read symbol");
    let file = SymbolFile::from_bytes(&bytes).expect("parse symbol");
    file.symbols[0]
        .pins
        .iter()
        .map(|p| p.name.clone())
        .collect()
}

#[test]
fn closing_a_library_with_a_draft_asks_and_lists_it() {
    let mut r = library_rig(1);
    let draft = dirty_symbol(&mut r);

    r.run(LibraryMessage::CloseLibrary(r.root.clone()));

    let prompt = r
        .app
        .library
        .close_library_confirm
        .as_ref()
        .expect("the prompt opens");
    assert_eq!(prompt.dirty_paths, vec![draft]);
    assert!(r.app.library.library_at(&r.root).is_some(), "still mounted");
}

#[test]
fn close_prompt_cancel_keeps_the_library_and_the_draft() {
    let mut r = library_rig(1);
    let draft = dirty_symbol(&mut r);
    let before = r.files();
    r.run(LibraryMessage::CloseLibrary(r.root.clone()));

    r.run(LibraryMessage::CloseLibraryConfirm(
        CloseLibraryChoice::Cancel,
    ));

    assert!(r.app.library.close_library_confirm.is_none());
    assert!(r.app.library.library_at(&r.root).is_some());
    assert!(r.app.document_state.is_document_dirty(&draft));
    assert_eq!(r.files(), before);
}

#[test]
fn discard_all_closes_the_library_and_leaves_the_file_as_saved() {
    let mut r = library_rig(1);
    let draft = dirty_symbol(&mut r);
    let before = r.files();
    r.run(LibraryMessage::CloseLibrary(r.root.clone()));

    r.run(LibraryMessage::CloseLibraryConfirm(
        CloseLibraryChoice::DiscardAll,
    ));

    assert!(r.app.library.library_at(&r.root).is_none());
    assert!(!r.app.document_state.is_document_dirty(&draft));
    assert!(!r.app.document_state.symbol_editors.contains_key(&draft));
    assert_eq!(r.files(), before, "nothing is written");
}

#[test]
fn save_all_writes_the_draft_then_closes_the_library() {
    let mut r = library_rig(1);
    let draft = dirty_symbol(&mut r);
    assert!(saved_pin_names(&draft).is_empty());
    r.run(LibraryMessage::CloseLibrary(r.root.clone()));

    r.run(LibraryMessage::CloseLibraryConfirm(
        CloseLibraryChoice::SaveAll,
    ));

    assert_eq!(saved_pin_names(&draft), ["EDITED"]);
    assert!(!r.app.document_state.is_document_dirty(&draft));
    assert!(r.app.library.library_at(&r.root).is_none());
}

// ---- m137 ----

fn request_delete(r: &mut LibRig, row_id: RowId) {
    r.run(LibraryMessage::BrowserDeleteRowRequest {
        library_path: r.root.clone(),
        table: TABLE.to_string(),
        row_id,
    });
}

#[test]
fn delete_asks_first_and_confirm_removes_only_that_row() {
    let mut r = library_rig(3);
    let (keep_a, gone, keep_b) = (r.rows[0], r.rows[1], r.rows[2]);
    r.select(gone);

    request_delete(&mut r, gone);
    let confirm = r.browser().delete_confirm.as_ref().expect("the confirm");
    assert_eq!(confirm.row_id, gone);
    assert_eq!(r.row_count(), 3, "nothing is deleted before the confirm");

    r.run(LibraryMessage::BrowserDeleteRowConfirm {
        library_path: r.root.clone(),
        table: TABLE.to_string(),
        row_id: gone,
    });

    assert!(r.browser().delete_confirm.is_none());
    assert!(r.row(gone).is_none());
    assert!(r.row(keep_a).is_some() && r.row(keep_b).is_some());
    assert_eq!(r.browser().selected_row, None, "the selection goes with it");
}

#[test]
fn delete_cancel_removes_nothing() {
    let mut r = library_rig(2);
    let id = r.rows[0];
    let before = r.files();
    request_delete(&mut r, id);

    r.run(LibraryMessage::BrowserDeleteRowCancel {
        library_path: r.root.clone(),
    });

    assert!(r.browser().delete_confirm.is_none());
    assert_eq!(r.row_count(), 2);
    assert_eq!(r.files(), before);
}

#[test]
fn a_read_only_library_opens_no_delete_confirm() {
    let mut r = read_only_rig(1);
    let id = r.rows[0];
    let before = r.files();

    request_delete(&mut r, id);

    assert!(r.browser().delete_confirm.is_none());
    assert_eq!(r.row_count(), 1);
    assert!(r.app.document_state.error_notice.is_some());
    assert_eq!(r.files(), before);
}

// ---- m138 ----

struct Missing {
    app: Signex,
    prj: PathBuf,
    _tmp: TempDir,
}

/// A project listing a library that is not on disk, its dialog open.
fn library_missing() -> Missing {
    let tmp = TempDir::new().expect("tempdir");
    let prj = tmp.path().join("Proj.snxprj");
    let mut data = project_data(tmp.path(), &[]);
    data.libraries.push(LibraryEntry {
        path: PathBuf::from("gone/gone.snxlib"),
        kind: LibraryEntryKind::ProjectLocal,
        library_id: None,
        unknown: Default::default(),
    });
    let mut app = fresh_app();
    load_project(&mut app, &prj, &data);
    let entry = &app.document_state.projects[0].data.libraries[0];
    let path = app.document_state.projects[0]
        .data
        .resolve_library_path(entry);
    app.library.recovery = Some(RecoveryDialog::LibraryMissing { path });
    Missing {
        app,
        prj,
        _tmp: tmp,
    }
}

impl Missing {
    fn choose(&mut self, choice: LibraryMissingChoice) {
        settle(
            &mut self.app,
            Message::Library(LibraryMessage::RecoveryLibraryMissing(choice)),
        );
    }

    fn listed(&self) -> usize {
        self.app.document_state.projects[0].data.libraries.len()
    }
}

#[test]
fn skip_for_now_on_a_missing_library_keeps_the_project_entry() {
    let mut m = library_missing();

    m.choose(LibraryMissingChoice::Cancel);

    assert!(m.app.library.recovery.is_none());
    assert_eq!(m.listed(), 1);
    assert!(!m.app.document_state.is_document_dirty(&m.prj));
}

#[test]
fn remove_from_project_drops_the_entry_as_an_unsaved_edit() {
    let mut m = library_missing();

    m.choose(LibraryMissingChoice::RemoveFromProject);

    assert!(m.app.library.recovery.is_none());
    assert_eq!(m.listed(), 0);
    assert!(m.app.document_state.is_document_dirty(&m.prj));
    assert_eq!(
        parse_project(&m.prj).expect("parse").libraries.len(),
        1,
        "the file waits for the project save"
    );
}

/// A library whose `.git/` holds no history, its open refused to the dialog.
fn git_missing() -> (Signex, PathBuf, TempDir) {
    let tmp = TempDir::new().expect("tempdir");
    let root = tmp.path().join("parts.snxlib");
    library_on_disk(&root, 1);
    std::fs::create_dir(tmp.path().join(".git")).expect("an empty .git");
    let mut app = fresh_app();
    settle(
        &mut app,
        Message::Library(LibraryMessage::OpenLibraryAt(Some(root.clone()))),
    );
    assert!(
        matches!(
            app.library.recovery,
            Some(RecoveryDialog::GitMissing { .. })
        ),
        "the open raises Git Repository Missing, got {:?}",
        app.library.recovery
    );
    (app, root, tmp)
}

#[test]
fn skip_for_now_on_missing_history_opens_the_library_read_only() {
    let (mut app, root, tmp) = git_missing();
    let before = files_under(tmp.path());

    settle(
        &mut app,
        Message::Library(LibraryMessage::RecoveryGitMissing(GitMissingChoice::Skip)),
    );

    assert!(app.library.recovery.is_none());
    assert!(app.library.library_at(&root).is_some(), "the library opens");
    assert!(app.library.is_read_only(&root));
    assert_eq!(files_under(tmp.path()), before, "nothing is written");
}

#[test]
fn closing_the_history_dialog_leaves_the_library_unopened() {
    let (mut app, root, tmp) = git_missing();
    let before = files_under(tmp.path());

    settle(
        &mut app,
        Message::Library(LibraryMessage::RecoveryGitMissing(GitMissingChoice::Cancel)),
    );

    assert!(app.library.recovery.is_none());
    assert!(app.library.library_at(&root).is_none());
    assert_eq!(files_under(tmp.path()), before);
}

fn broken_binding(r: &mut LibRig, row_id: RowId) {
    r.app.library.recovery = Some(RecoveryDialog::BrokenPrimitiveBinding {
        library_path: r.root.clone(),
        table: TABLE.to_string(),
        row_id: row_id.as_uuid(),
        internal_pn: None,
        kind: PrimitiveKind::Footprint,
        missing_uuid: Uuid::now_v7(),
    });
}

#[test]
fn skip_for_now_on_a_broken_binding_keeps_the_row() {
    let mut r = library_rig(1);
    let id = r.rows[0];
    let before = r.files();
    broken_binding(&mut r, id);

    r.run(LibraryMessage::RecoveryBrokenBinding(
        BrokenBindingChoice::Cancel,
    ));

    assert!(r.app.library.recovery.is_none());
    assert!(r.row(id).is_some());
    assert_eq!(r.files(), before);
}

/// Row m138: Remove row deletes the row whose binding is broken.
#[test]
fn remove_row_on_a_broken_binding_deletes_that_row() {
    let mut r = library_rig(2);
    let (gone, kept) = (r.rows[0], r.rows[1]);
    broken_binding(&mut r, gone);

    r.run(LibraryMessage::RecoveryBrokenBinding(
        BrokenBindingChoice::RemoveRow,
    ));

    assert!(r.app.library.recovery.is_none());
    assert!(r.row(gone).is_none(), "the broken row is removed");
    assert!(r.row(kept).is_some());
}

/// Row m138: Re-bind… opens the primitive picker for the broken row.
#[test]
fn rebind_on_a_broken_binding_opens_the_picker_for_that_row() {
    let mut r = library_rig(1);
    let id = r.rows[0];
    broken_binding(&mut r, id);

    r.run(LibraryMessage::RecoveryBrokenBinding(
        BrokenBindingChoice::Rebind,
    ));

    let picker = r
        .app
        .library
        .primitive_picker
        .as_ref()
        .expect("Re-bind opens the picker");
    assert_eq!(picker.kind, PrimitiveKind::Footprint);
    assert!(matches!(
        &picker.target,
        PrimitivePickerTarget::BrowserRow(address) if address.row_id == id
    ));
}
