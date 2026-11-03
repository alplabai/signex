//! #1854: the `.snxprj` reads clean again when the three Project-tab
//! histories (General, rules, net classes) undo back to the saved state, and
//! never otherwise.

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, ProjectMsg, ProjectTabMsg, Signex};

fn app() -> Signex {
    let dir = std::env::temp_dir().join(format!("signex-prj-sp-{}", uuid::Uuid::now_v7()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["a_root.snxsch"]);
    app.ui_state.erc_severity_override.clear();
    app.ui_state.pin_matrix_overrides.clear();
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

fn parameter(app: &mut Signex, name: &str, value: &str) {
    let project_id = app.document_state.projects[0].id;
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::ParameterEdited {
            project_id,
            name: name.to_string(),
            value: value.to_string(),
        },
    )));
}

fn dirty(app: &Signex) -> bool {
    let path = &app.document_state.projects[0].path;
    app.document_state.is_document_dirty(path)
}

fn step(app: &mut Signex, undo: bool) -> bool {
    let id = app.document_state.projects[0].id;
    app.step_project_tab_history(id, undo)
}

#[test]
fn undo_back_to_the_saved_state_is_clean_and_redo_is_dirty() {
    let mut app = app();
    assert!(!dirty(&app));
    describe(&mut app, "a");
    assert!(dirty(&app));
    assert!(step(&mut app, true));
    assert!(!dirty(&app), "undo returned to the file's content");
    assert!(step(&mut app, false));
    assert!(dirty(&app));
}

#[test]
fn a_typing_run_is_one_step_and_undoes_to_clean() {
    let mut app = app();
    for text in ["a", "ab", "abc"] {
        describe(&mut app, text);
    }
    assert!(step(&mut app, true));
    assert!(!dirty(&app));
}

#[test]
fn undoing_only_one_of_two_histories_stays_dirty() {
    let mut app = app();
    let _ = app.handle_store_erc_rules_in_project();
    describe(&mut app, "kept");
    assert!(step(&mut app, true));
    assert!(dirty(&app), "the stored rules are still unsaved");
    assert!(app.undo_project_rules());
    assert!(!dirty(&app), "both histories are back at the saved state");
}

#[test]
fn an_outside_mark_survives_undo_until_a_save() {
    let mut app = app();
    let path = app.document_state.projects[0].path.clone();
    app.document_state.mark_document_dirty(&path);
    describe(&mut app, "a");
    assert!(step(&mut app, true));
    assert!(dirty(&app), "a non-history edit is never hidden by undo");
    app.document_state.mark_document_clean(&path);
    assert!(!dirty(&app));
}

#[test]
fn an_undo_while_a_write_is_in_flight_stays_dirty() {
    let mut app = app();
    let path = app.document_state.projects[0].path.clone();
    describe(&mut app, "a");
    let _job = app.document_state.document_saves.begin(path, Vec::new());
    assert!(step(&mut app, true));
    assert!(dirty(&app));
}

#[test]
fn a_failed_write_forgets_the_saved_state() {
    let mut app = app();
    let path = app.document_state.projects[0].path.clone();
    describe(&mut app, "a");
    app.document_state.forget_project_saved_state(&path);
    assert!(step(&mut app, true));
    assert!(dirty(&app));
}

#[test]
fn a_history_that_evicted_the_saved_state_stays_dirty() {
    let mut app = app();
    let id = app.document_state.projects[0].id;
    for n in 0..=signex_engine::PROJECT_GENERAL_HISTORY_LIMIT {
        parameter(&mut app, "Rev", &n.to_string());
        if let Some(tab) = app.document_state.project_tabs.get_mut(&id) {
            tab.general_history.close_run();
        }
    }
    while step(&mut app, true) {}
    assert!(
        dirty(&app),
        "the oldest step fell off; no step reaches the file"
    );
}

#[test]
fn saving_then_undoing_to_the_saved_state_is_clean() {
    let mut app = app();
    let path = app.document_state.projects[0].path.clone();
    describe(&mut app, "a");
    // A save ends the typing run (`plan_project_write`), so "b" is a new step.
    let id = app.document_state.projects[0].id;
    app.document_state
        .project_tabs
        .get_mut(&id)
        .unwrap()
        .general_history
        .close_run();
    app.document_state.mark_document_clean(&path);
    describe(&mut app, "b");
    assert!(dirty(&app));
    assert!(step(&mut app, true));
    assert!(!dirty(&app), "back to what the save wrote");
}
