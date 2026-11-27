//! Restoring a project copy drops the Project-tab undo steps recorded
//! against the data it replaces (#2975): an undo after the restore must not
//! write stale values over the restored project.

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, ProjectMsg, ProjectTabMsg, Signex};

fn app() -> Signex {
    let dir = std::env::temp_dir().join(format!("signex-prj-restore-{}", uuid::Uuid::now_v7()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["a_root.snxsch"]);
    app.open_project_tab(0);
    app
}

fn describe(app: &mut Signex, text: &str) {
    let project_id = app.document_state.projects[0].id;
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::DescriptionEdited {
            project_id,
            text: text.to_string(),
        },
    )));
}

fn restore(app: &mut Signex, description: &str) {
    let path = app.document_state.projects[0].path.clone();
    let mut copy = app.document_state.projects[0].data.clone();
    copy.description = Some(description.to_string());
    assert!(app.restore_project_copy(&path, copy));
}

#[test]
fn a_restored_project_copy_has_no_general_undo_step() {
    let mut app = app();
    let id = app.document_state.projects[0].id;
    describe(&mut app, "edited");
    assert!(app.document_state.general_history.contains_key(&id));

    restore(&mut app, "restored");

    assert!(!app.document_state.general_history.contains_key(&id));
    assert!(!app.step_project_tab_history(id, true), "nothing to undo");
    assert_eq!(
        app.document_state.projects[0].data.description.as_deref(),
        Some("restored"),
        "undo left the restored General values alone"
    );
}

#[test]
fn a_restored_project_copy_has_no_rules_undo_step() {
    let mut app = app();
    let id = app.document_state.projects[0].id;
    let _ = app.handle_store_erc_rules_in_project();
    assert!(app.document_state.rules_history.contains_key(&id));

    restore(&mut app, "restored");

    assert!(!app.document_state.rules_history.contains_key(&id));
    assert!(!app.undo_project_rules(), "nothing to undo");
}
