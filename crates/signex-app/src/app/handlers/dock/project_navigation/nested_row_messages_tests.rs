//! A refused Remove reaches the Messages panel, and a directory is not held
//! back by incomplete sheet data (#1618).

use std::path::PathBuf;

use super::*;
use crate::app::handlers::menu::export::tests::{app_workspace, open_with, sheet_with_net};
use crate::app::runtime::sheet_tree_tests::{settle, write_sheet};
use crate::app::{ProjectTreeAction, RemoveChoice, RemoveDialogState};

#[test]
fn a_refused_exclude_reaches_the_messages_panel() {
    let dir = "/w/nested-messages";
    let mut app = app_workspace(dir, &["top.snxsch"]);
    open_with(
        &mut app.document_state,
        &PathBuf::from(dir).join("top.snxsch"),
        sheet_with_net("R_TOP", "N", &["child-msgs.snxsch"]),
    );
    open_with(
        &mut app.document_state,
        &PathBuf::from(dir).join("child-msgs.snxsch"),
        sheet_with_net("R_CHILD", "M", &[]),
    );
    settle(&mut app, false);
    let key = vec![
        app.document_state.projects[0].id.to_string(),
        "Source Documents".to_string(),
        "top.snxsch".to_string(),
        "child-msgs.snxsch".to_string(),
    ];
    let opened = app.handle_project_tree_action(ProjectTreeAction::OpenRemoveDialog(key));
    assert_eq!(opened.units(), 0);

    let confirmed = app.handle_remove_confirm(RemoveChoice::ExcludeFromProject);

    assert_eq!(confirmed.units(), 0);
    assert!(
        app.document_state
            .panel_ctx
            .diagnostics
            .iter()
            .any(|e| e.message.contains("nothing to exclude")),
        "the refusal is in the panel snapshot, not only the ring"
    );
}

#[test]
fn a_library_directory_is_deleted_although_other_projects_are_unread() {
    let tmp = tempfile::tempdir().expect("tempdir");
    write_sheet(tmp.path(), "top.snxsch", &[]);
    let lib = tmp.path().join("parts.snxlib");
    std::fs::create_dir(&lib).expect("mkdir");
    let dir = tmp.path().to_string_lossy().into_owned();
    let mut app = app_workspace(&dir, &["top.snxsch"]);
    // Not walked yet: for a file that would mean "cannot be ruled out".
    app.refresh_panel_ctx();
    let id = app.document_state.projects[0].id;
    app.ui_state.remove_dialog = Some(RemoveDialogState {
        target_path: lib.clone(),
        tree_path: vec![0, 1, 0],
        project_id: Some(id),
        display_name: "parts.snxlib".to_string(),
        is_outside_project: false,
        unsaved_prompt: false,
    });
    assert!(
        app.remove_blockers(app.ui_state.remove_dialog.as_ref().expect("set"))
            .unknown
            .is_some()
    );

    let _deleted = app.handle_remove_confirm(RemoveChoice::DeleteFile);

    assert!(!lib.exists(), "the directory package is deleted");
}
