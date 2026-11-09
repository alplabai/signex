//! #2232: the Project setup section edits the defaults a project was created
//! with, as a normal project edit: it dirties the `.snxprj`, writes nothing
//! until Save, and undoes.

use signex_types::project::{PcbField, SetupProfile};

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, NewProjectMsg, ProjectMsg, ProjectSection, ProjectTabMsg, Signex};

fn app() -> Signex {
    let dir = std::env::temp_dir().join(format!("signex-prj-setup-{}", uuid::Uuid::now_v7()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["root.snxsch"]);
    app.open_project_tab(0);
    let project_id = app.document_state.projects[0].id;
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::SectionSelected {
            project_id,
            section: ProjectSection::Setup,
        },
    )));
    app
}

fn edit(app: &mut Signex, msg: NewProjectMsg) {
    let project_id = app.document_state.projects[0].id;
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::SetupEdited { project_id, msg },
    )));
}

fn project_dirty(app: &Signex) -> bool {
    let path = &app.document_state.projects[0].path;
    app.document_state.is_document_dirty(path)
}

#[test]
fn a_profile_and_a_clearance_are_stored_as_a_project_edit() {
    let mut app = app();
    let project_path = app.document_state.projects[0].path.clone();
    let on_disk = std::fs::read(&project_path).ok();
    assert!(!project_dirty(&app));

    edit(&mut app, NewProjectMsg::ProfilePicked(SetupProfile::Hobby));
    edit(
        &mut app,
        NewProjectMsg::RuleEdited(PcbField::Clearance, "0.12".into()),
    );

    let stored = &app.document_state.projects[0].data.project_defaults;
    assert_eq!(stored.pcb.as_ref().map(|p| p.copper_layers), Some(2));
    assert_eq!(stored.pcb.as_ref().map(|p| p.clearance_mm), Some(0.12));
    assert!(project_dirty(&app), "the .snxprj is dirty");
    assert_eq!(
        std::fs::read(&project_path).ok(),
        on_disk,
        "nothing is written before Save"
    );
}

#[test]
fn undo_and_a_bad_value_leave_the_stored_defaults_unchanged() {
    let mut app = app();
    edit(&mut app, NewProjectMsg::ProfilePicked(SetupProfile::Hdi));
    let after_pick = app.document_state.projects[0].data.project_defaults.clone();
    assert_eq!(after_pick.pcb.as_ref().map(|p| p.copper_layers), Some(6));

    // A value that does not parse stays a draft with its reason.
    edit(
        &mut app,
        NewProjectMsg::RuleEdited(PcbField::Clearance, "abc".into()),
    );
    assert_eq!(
        app.document_state.projects[0].data.project_defaults,
        after_pick
    );

    // Undo takes the section back to what the project stored before.
    let id = app.document_state.projects[0].id;
    assert!(app.step_project_tab_history(id, true));
    assert!(
        app.document_state.projects[0]
            .data
            .project_defaults
            .is_empty()
    );
    assert!(!project_dirty(&app), "undo returned to the saved state");
}

#[test]
fn the_section_builds_from_the_stored_values() {
    let app = app();
    let id = app.document_state.projects[0].id;
    let _ = app.view_project_tab(id);
}
