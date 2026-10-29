//! #1080 — project parameters: edited in the Project tab (undoable, saved
//! with the project), inherited by every sheet, overridden per sheet, and
//! cleared back to the project's value.

use signex_app::app::{EditMsg, Message, ProjectMsg, ProjectTabMsg, Signex};
use signex_app::dock::DockMessage;
use signex_app::panels::{PanelMsg, TitleBlockField};
use signex_output::template::FramePrimitive;

use crate::project_tab_support::{
    fixture, is_dirty, on_disk, open_dirty_root_sheet, open_tab, project_id, save,
};

fn set_parameter(app: &mut Signex, name: &str, value: &str) {
    let project_id = project_id(app);
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::ParameterEdited {
            project_id,
            name: name.to_string(),
            value: value.to_string(),
        },
    )));
}

fn set_sheet_company(app: &mut Signex, text: &str) {
    let field = TitleBlockField::Company;
    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::TitleBlockTyping(field, text.to_string()),
    )));
    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::CommitTitleBlockField(field),
    )));
}

fn drawn(app: &Signex) -> Vec<String> {
    app.interaction_state
        .active_canvas()
        .sheet_frame
        .iter()
        .filter_map(|p| match p {
            FramePrimitive::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn parameters_are_dirty_undoable_and_saved_with_the_project() {
    let mut f = fixture();
    open_tab(&mut f.app);
    set_parameter(&mut f.app, "company", "Acme");
    assert!(is_dirty(&f));
    let _ = f.app.update(Message::Edit(EditMsg::Undo));
    assert!(f.app.document_state.projects[0].data.parameters.is_empty());
    let _ = f.app.update(Message::Edit(EditMsg::Redo));

    set_parameter(&mut f.app, "Voltage", "24 V");
    save(&mut f.app);
    let saved = on_disk(&f.prj).parameters;
    assert_eq!(saved.get("company").map(String::as_str), Some("Acme"));
    assert_eq!(saved.get("Voltage").map(String::as_str), Some("24 V"));
}

#[test]
fn a_sheet_inherits_the_project_value_overrides_it_and_falls_back_when_cleared() {
    let mut f = fixture();
    open_tab(&mut f.app);
    f.app.document_state.projects[0]
        .data
        .sheets
        .push(signex_types::project::SheetEntry {
            name: "Oscar".into(),
            filename: "Oscar.snxsch".into(),
            symbols_count: 0,
            wires_count: 0,
            labels_count: 0,
            unknown: Default::default(),
        });
    open_dirty_root_sheet(&mut f);
    set_parameter(&mut f.app, "company", "Acme");
    assert!(
        drawn(&f.app).contains(&"Acme".to_string()),
        "inherited {:?}",
        drawn(&f.app)
    );

    set_sheet_company(&mut f.app, "Local Co");
    let texts = drawn(&f.app);
    assert!(texts.contains(&"Local Co".to_string()), "overridden");
    assert!(!texts.contains(&"Acme".to_string()));

    set_sheet_company(&mut f.app, "");
    assert!(
        drawn(&f.app).contains(&"Acme".to_string()),
        "clearing the override falls back to the project"
    );
}
