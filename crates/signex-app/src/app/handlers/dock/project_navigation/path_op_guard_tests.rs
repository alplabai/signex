//! #859 — a rename or delete worker owns its paths. These pin the refusals
//! that keep a second move, a delete, an open or a save away from them:
//! while a project folder is being renamed, while another path op overlaps a
//! project rename, and while a delete is in flight.

use super::project_rename_tests::{Fixture, fixture, rename_error, submit_rename};
use crate::app::path_ops::PathOpKind;
use crate::app::state::scope::CaseFold;
use crate::app::{
    FileMsg, Message, RemoveChoice, RemoveDialogState, RemoveMsg, RenameDialogState, RenameMsg,
};

/// A fixture whose project folder rename has started but not finished: the
/// worker task is returned and dropped unrun, so the move stays in flight.
fn folder_move_in_flight(root: &std::path::Path) -> Fixture {
    let mut fx = fixture(root, "Foo", false);
    let file = fx.folder.join("Foo.snxprj");
    drop(submit_rename(&mut fx.app, &file, "Bar"));
    assert!(
        !fx.app.document_state.project_folder_renames.is_empty(),
        "the folder move is in flight"
    );
    fx
}

fn remove_dialog_for(fx: &Fixture) -> RemoveDialogState {
    RemoveDialogState {
        project_id: None,
        target_path: fx.sheet.clone(),
        tree_path: vec![0, 0],
        display_name: "main.snxsch".to_string(),
        is_outside_project: false,
        unsaved_prompt: false,
    }
}

fn file_rename_dialog_for(fx: &Fixture, new_name: &str) -> RenameDialogState {
    RenameDialogState {
        project_id: None,
        target_path: fx.sheet.clone(),
        tree_path: vec![0, 0],
        buffer: new_name.to_string(),
        error: None,
        is_project_rename: false,
    }
}

#[test]
fn a_file_rename_is_refused_while_a_project_folder_move_is_in_flight() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = folder_move_in_flight(root.path());
    fx.app.ui_state.rename_dialog = Some(file_rename_dialog_for(&fx, "other.snxsch"));

    drop(fx.app.update(Message::Rename(RenameMsg::Submit)));

    assert!(rename_error(&fx).contains("project folder is being renamed"));
    assert!(fx.app.document_state.pending_path_ops.is_empty());
    assert!(fx.sheet.exists(), "nothing was moved");
}

#[test]
fn a_delete_is_refused_while_a_project_folder_move_is_in_flight() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = folder_move_in_flight(root.path());
    fx.app.ui_state.remove_dialog = Some(remove_dialog_for(&fx));

    drop(fx.app.update(Message::Remove(RemoveMsg::Confirm(
        RemoveChoice::DeleteFile,
    ))));

    let notice = fx.app.document_state.error_notice.clone().expect("notice");
    assert!(notice.detail.contains("project folder is being renamed"));
    assert!(fx.app.document_state.pending_path_ops.is_empty());
    assert!(fx.sheet.exists(), "nothing was deleted");
    assert_eq!(fx.app.document_state.projects[0].data.sheets.len(), 1);
}

#[test]
fn a_project_rename_is_refused_while_a_path_op_runs_under_its_folder() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    fx.app.document_state.pending_path_ops.begin(
        PathOpKind::Rename,
        vec![fx.sheet.clone(), fx.folder.join("moved.snxsch")],
        CaseFold::PRESERVE,
    );
    let file = fx.folder.join("Foo.snxprj");

    drop(submit_rename(&mut fx.app, &file, "Bar"));

    assert!(rename_error(&fx).contains("is still running, so"));
    assert!(fx.app.document_state.project_folder_renames.is_empty());
    assert!(fx.folder.is_dir(), "the folder kept its name");
}

/// Between Confirm and the completion the entry is still in the project, but
/// the file is owned by the delete: it cannot be opened, renamed again or
/// saved, and a second delete is refused too.
#[test]
fn a_file_whose_delete_is_in_flight_cannot_be_opened_renamed_or_saved() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    fx.app.ui_state.remove_dialog = Some(remove_dialog_for(&fx));
    let delete = fx.app.update(Message::Remove(RemoveMsg::Confirm(
        RemoveChoice::DeleteFile,
    )));
    assert!(!fx.app.document_state.pending_path_ops.is_empty());
    assert_eq!(
        fx.app.document_state.projects[0].data.sheets.len(),
        1,
        "the entry stays until the completion prunes it"
    );

    drop(
        fx.app
            .update(Message::File(FileMsg::Opened(Some(fx.sheet.clone())))),
    );
    assert!(
        fx.app
            .document_state
            .tabs
            .iter()
            .all(|t| t.path != fx.sheet),
        "the file was not opened"
    );

    fx.app.ui_state.rename_dialog = Some(file_rename_dialog_for(&fx, "other.snxsch"));
    drop(fx.app.update(Message::Rename(RenameMsg::Submit)));
    assert!(rename_error(&fx).contains("A delete of main.snxsch is still running"));

    assert!(fx.app.refuse_while_path_op(&fx.sheet, "saved").is_err());

    fx.app.ui_state.remove_dialog = Some(remove_dialog_for(&fx));
    drop(fx.app.update(Message::Remove(RemoveMsg::Confirm(
        RemoveChoice::DeleteFile,
    ))));
    assert!(fx.app.document_state.error_notice.is_some());
    drop(delete);
}

/// A delete under the project is pending: the project's own file must not be
/// written (it would still list the file) and the project must not close (the
/// completion could no longer prune its entry).
fn delete_pending_under_project(fx: &mut Fixture) {
    fx.app.document_state.pending_path_ops.begin(
        PathOpKind::Delete,
        vec![fx.sheet.clone()],
        CaseFold::PRESERVE,
    );
}

#[test]
fn a_project_save_is_refused_while_a_delete_under_it_is_pending() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    delete_pending_under_project(&mut fx);
    let id = fx.app.document_state.projects[0].id;

    let err = fx.app.persist_project_by_id(id).unwrap_err();

    assert!(format!("{err:#}").contains("delete"), "{err:#}");
}

#[test]
fn close_project_is_held_while_a_delete_under_it_is_pending() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut fx = fixture(root.path(), "Foo", false);
    delete_pending_under_project(&mut fx);

    drop(fx.app.close_project_at_tree_path(&[0]));

    assert_eq!(fx.app.document_state.projects.len(), 1, "still open");
    let notice = fx.app.document_state.error_notice.clone().expect("notice");
    assert!(notice.detail.contains("delete"), "{}", notice.detail);
}
