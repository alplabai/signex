//! Rows m23 (Rename) and m24 (Remove from project).

use signex_app::app::{Message, ProjectTreeAction, RemoveChoice, RemoveMsg, RenameMsg};

use super::support::{Bench, bench};

/// `root` places `child`; `b` is a page nothing places.
const PLACED: &[(&str, &str)] = &[
    ("root.snxsch", "child.snxsch"),
    ("child.snxsch", ""),
    ("b.snxsch", ""),
];

fn open_rename(b: &mut Bench, file: &str) {
    let key = b.row(file);
    b.action(ProjectTreeAction::OpenRenameDialog(key));
    assert!(b.app.ui_state.rename_dialog.is_some(), "the dialog opens");
}

fn type_name(b: &mut Bench, name: &str) {
    b.send(Message::Rename(RenameMsg::BufferChanged(name.to_string())));
}

fn rename_error(b: &Bench) -> Option<String> {
    b.app
        .ui_state
        .rename_dialog
        .as_ref()
        .and_then(|dialog| dialog.error.clone())
}

#[test]
fn m23_the_dialog_targets_the_file_and_names_the_symbol_that_places_it() {
    let mut b = bench(PLACED);
    open_rename(&mut b, "child.snxsch");
    let dialog = b.app.ui_state.rename_dialog.as_ref().expect("the dialog");
    assert_eq!(dialog.target_path, b.file("child.snxsch"));
    assert_eq!(dialog.buffer, "child.snxsch");
    assert!(!dialog.is_project_rename);
    assert_eq!(dialog.placing_symbols.len(), 1);
}

#[test]
fn m23_rename_moves_the_file_and_the_placing_symbol_follows() {
    let mut b = bench(PLACED);
    open_rename(&mut b, "child.snxsch");
    type_name(&mut b, "power.snxsch");
    b.send(Message::Rename(RenameMsg::Submit));
    assert!(b.app.ui_state.rename_dialog.is_none(), "the rename ran");
    assert!(b.file("power.snxsch").is_file());
    assert!(!b.file("child.snxsch").exists());
    assert!(b.listed().contains(&"power.snxsch".to_string()));
    assert!(!b.listed().contains(&"child.snxsch".to_string()));
    assert_eq!(
        b.sheet("root.snxsch").child_sheets[0].filename,
        "power.snxsch"
    );
}

#[test]
fn m23_cancel_changes_nothing() {
    let mut b = bench(PLACED);
    open_rename(&mut b, "child.snxsch");
    type_name(&mut b, "power.snxsch");
    b.send(Message::Rename(RenameMsg::Close));
    assert!(b.app.ui_state.rename_dialog.is_none());
    assert!(b.file("child.snxsch").is_file());
    assert!(!b.file("power.snxsch").exists());
    assert!(b.listed().contains(&"child.snxsch".to_string()));
    assert_eq!(
        b.sheet("root.snxsch").child_sheets[0].filename,
        "child.snxsch"
    );
}

#[test]
fn m23_a_name_another_file_has_shows_the_clash_and_rename_does_nothing() {
    let mut b = bench(PLACED);
    let before = std::fs::read(b.file("b.snxsch")).expect("file");
    open_rename(&mut b, "child.snxsch");
    type_name(&mut b, "b.snxsch");
    let dialog = b.app.ui_state.rename_dialog.as_ref().expect("the dialog");
    assert_eq!(dialog.name_clash.as_deref(), Some("b.snxsch"));
    b.send(Message::Rename(RenameMsg::Submit));
    assert!(b.app.ui_state.rename_dialog.is_some(), "the dialog stays");
    assert!(b.file("child.snxsch").is_file());
    assert_eq!(std::fs::read(b.file("b.snxsch")).expect("file"), before);
}

#[test]
fn m23_clearing_the_clashing_name_clears_the_clash() {
    let mut b = bench(PLACED);
    open_rename(&mut b, "child.snxsch");
    type_name(&mut b, "b.snxsch");
    type_name(&mut b, "free.snxsch");
    let dialog = b.app.ui_state.rename_dialog.as_ref().expect("the dialog");
    assert!(dialog.name_clash.is_none());
}

#[test]
fn m23_a_name_that_changes_the_extension_is_refused() {
    let mut b = bench(PLACED);
    open_rename(&mut b, "child.snxsch");
    type_name(&mut b, "child.txt");
    b.send(Message::Rename(RenameMsg::Submit));
    assert!(rename_error(&b).is_some(), "the refusal says why");
    assert!(b.file("child.snxsch").is_file());
    assert!(!b.file("child.txt").exists());
}

#[test]
fn m23_an_empty_name_and_a_path_are_refused() {
    let mut b = bench(PLACED);
    open_rename(&mut b, "child.snxsch");
    for bad in ["", "../escape.snxsch"] {
        type_name(&mut b, bad);
        b.send(Message::Rename(RenameMsg::Submit));
        assert!(rename_error(&b).is_some(), "{bad:?} is refused");
        assert!(b.file("child.snxsch").is_file());
    }
}

#[test]
fn m23_rename_project_renames_the_project_file_and_its_name() {
    let mut b = bench(PLACED);
    let root = b.root_key();
    b.action(ProjectTreeAction::OpenProjectRenameDialog(root));
    let dialog = b.app.ui_state.rename_dialog.as_ref().expect("the dialog");
    assert!(dialog.is_project_rename);
    assert_eq!(dialog.buffer, "Proj");
    type_name(&mut b, "Renamed");
    b.send(Message::Rename(RenameMsg::Submit));
    let renamed = b.file("Renamed.snxprj");
    assert!(renamed.is_file());
    assert!(!b.prj.exists());
    assert_eq!(b.app.document_state.projects[0].path, renamed);
    assert_eq!(b.data().name, "Renamed");
}

#[test]
fn m23_rename_project_refuses_a_name_with_a_dot() {
    let mut b = bench(PLACED);
    let root = b.root_key();
    b.action(ProjectTreeAction::OpenProjectRenameDialog(root));
    type_name(&mut b, "bad.name");
    b.send(Message::Rename(RenameMsg::Submit));
    assert!(rename_error(&b).is_some());
    assert!(b.prj.is_file());
    assert_eq!(b.data().name, "Proj");
}

fn open_remove(b: &mut Bench, file: &str) {
    let key = b.row(file);
    b.action(ProjectTreeAction::OpenRemoveDialog(key));
    assert!(b.app.ui_state.remove_dialog.is_some(), "the dialog opens");
}

#[test]
fn m24_picking_a_card_selects_it_and_removes_nothing() {
    let mut b = bench(PLACED);
    open_remove(&mut b, "b.snxsch");
    b.send(Message::Remove(RemoveMsg::ChoicePicked(
        RemoveChoice::DeleteFile,
    )));
    let dialog = b.app.ui_state.remove_dialog.as_ref().expect("the dialog");
    assert_eq!(dialog.choice, Some(RemoveChoice::DeleteFile));
    assert!(b.file("b.snxsch").is_file());
    assert!(b.listed().contains(&"b.snxsch".to_string()));
}

#[test]
fn m24_exclude_drops_the_entry_keeps_the_file_and_dirties_the_project() {
    let mut b = bench(PLACED);
    open_remove(&mut b, "b.snxsch");
    b.send(Message::Remove(RemoveMsg::Confirm(
        RemoveChoice::ExcludeFromProject,
    )));
    assert!(b.app.ui_state.remove_dialog.is_none());
    assert!(!b.listed().contains(&"b.snxsch".to_string()));
    assert!(b.file("b.snxsch").is_file(), "the file stays in the folder");
    assert!(b.dirty(), "the project list changed and is not saved");
}

#[test]
fn m24_delete_removes_the_file_and_its_saved_entry() {
    let mut b = bench(PLACED);
    open_remove(&mut b, "b.snxsch");
    b.send(Message::Remove(RemoveMsg::Confirm(
        RemoveChoice::DeleteFile,
    )));
    assert!(!b.file("b.snxsch").exists());
    assert!(!b.listed().contains(&"b.snxsch".to_string()));
    let saved: Vec<String> = b.on_disk().sheet_order();
    assert!(!saved.contains(&"b.snxsch".to_string()));
}

#[test]
fn m24_cancel_removes_nothing() {
    let mut b = bench(PLACED);
    open_remove(&mut b, "b.snxsch");
    b.send(Message::Remove(RemoveMsg::ChoicePicked(
        RemoveChoice::DeleteFile,
    )));
    b.send(Message::Remove(RemoveMsg::Close));
    assert!(b.app.ui_state.remove_dialog.is_none());
    assert!(b.file("b.snxsch").is_file());
    assert!(b.listed().contains(&"b.snxsch".to_string()));
    assert!(!b.dirty());
}

#[test]
fn m24_excluding_a_sheet_with_unsaved_edits_asks_first_and_cancel_keeps_it() {
    let mut b = bench(PLACED);
    let path = b.file("b.snxsch");
    b.send(Message::File(signex_app::app::FileMsg::Opened(Some(
        path.clone(),
    ))));
    b.app.document_state.mark_document_dirty(&path);
    open_remove(&mut b, "b.snxsch");
    b.send(Message::Remove(RemoveMsg::Confirm(
        RemoveChoice::ExcludeFromProject,
    )));
    let dialog = b.app.ui_state.remove_dialog.as_ref().expect("still open");
    assert!(dialog.unsaved_prompt, "it moves to the unsaved-edits stage");
    assert!(b.listed().contains(&"b.snxsch".to_string()));
    b.send(Message::Remove(RemoveMsg::Close));
    assert!(b.listed().contains(&"b.snxsch".to_string()));
    assert!(b.app.document_state.is_document_dirty(&path));
}

#[test]
fn m24_delete_is_refused_while_another_sheet_places_the_file() {
    let mut b = bench(&[
        ("root.snxsch", "shared.snxsch"),
        ("other.snxsch", "shared.snxsch"),
        ("shared.snxsch", ""),
    ]);
    open_remove(&mut b, "shared.snxsch");
    b.send(Message::Remove(RemoveMsg::Confirm(
        RemoveChoice::DeleteFile,
    )));
    assert!(b.file("shared.snxsch").is_file(), "the file survives");
    assert!(b.listed().contains(&"shared.snxsch".to_string()));
    assert!(
        b.app.document_state.error_notice.is_some(),
        "the refusal is shown"
    );
}
