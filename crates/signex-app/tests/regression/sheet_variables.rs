//! #1987 — sheet and project variables: a sheet variable is edited in the
//! sheet's Properties, shadows the project variable of the same name, and is
//! saved as its source text; the title block draws the evaluated value; a
//! variable cycle is reported in Messages.

use signex_app::app::{Message, ProjectMsg, ProjectTabMsg, Signex};
use signex_app::dock::DockMessage;
use signex_app::panels::{PanelMsg, SheetVariableMsg};
use signex_output::template::FramePrimitive;

use crate::project_tab_support::{fixture, open_dirty_root_sheet, open_tab, project_id, save};

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

fn sheet_variables(app: &mut Signex, edit: SheetVariableMsg) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::SheetVariables(
        edit,
    ))));
}

/// Add the sheet variable `name` and enter `value` for it, as the panel does.
fn set_sheet_variable(app: &mut Signex, name: &str, value: &str) {
    sheet_variables(app, SheetVariableMsg::NewNameTyped(name.to_string()));
    sheet_variables(app, SheetVariableMsg::Added);
    sheet_variables(
        app,
        SheetVariableMsg::ValueTyped {
            name: name.to_string(),
            text: value.to_string(),
        },
    );
    sheet_variables(app, SheetVariableMsg::ValueSubmitted(name.to_string()));
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

fn logged(needle: &str) -> usize {
    drop(signex_app::diagnostics::init_logging());
    signex_app::diagnostics::recent_entries()
        .iter()
        .filter(|e| e.message.contains(needle))
        .count()
}

#[test]
fn a_sheet_variable_shadows_the_project_and_is_saved_as_its_source() {
    let mut f = fixture();
    open_tab(&mut f.app);
    let sheet = open_dirty_root_sheet(&mut f);
    set_parameter(&mut f.app, "VCC", "5V");
    set_parameter(&mut f.app, "company", "= VCC / 2");
    assert!(
        drawn(&f.app).contains(&"2.5V".to_string()),
        "the title block draws the project expression's value: {:?}",
        drawn(&f.app)
    );

    set_sheet_variable(&mut f.app, "VCC", "3.3V");
    let texts = drawn(&f.app);
    assert!(texts.contains(&"1.65V".to_string()), "shadowed: {texts:?}");
    assert!(!texts.contains(&"= VCC / 2".to_string()));

    // The source is what is stored, in the project and in the sheet.
    let project = &f.app.document_state.projects[0].data;
    assert_eq!(
        project.parameters.get("company").map(String::as_str),
        Some("= VCC / 2")
    );
    let engine = &f.app.document_state.engines[&sheet];
    assert_eq!(
        engine.document().variables.get("VCC").map(String::as_str),
        Some("3.3V")
    );

    save(&mut f.app);
    let on_disk = std::fs::read_to_string(&sheet).expect("saved sheet");
    assert!(on_disk.contains("[extras.sheet.variables]"), "{on_disk}");
    assert!(on_disk.contains("VCC = \"3.3V\""), "{on_disk}");
    assert!(!on_disk.contains("1.65"), "{on_disk}");
}

#[test]
fn a_sheet_variable_cycle_is_reported_in_messages() {
    let mut f = fixture();
    open_tab(&mut f.app);
    open_dirty_root_sheet(&mut f);
    set_sheet_variable(&mut f.app, "CYCLE_A_1987", "= CYCLE_B_1987 + 1");
    set_sheet_variable(&mut f.app, "CYCLE_B_1987", "= CYCLE_A_1987 * 2");
    assert!(
        logged("Sheet variable 'CYCLE_A_1987'") > 0,
        "the cycle reaches the Messages panel"
    );
    assert!(logged("cycle") > 0);
}

#[test]
fn a_name_that_is_not_an_identifier_is_refused() {
    let mut f = fixture();
    open_tab(&mut f.app);
    let sheet = open_dirty_root_sheet(&mut f);
    sheet_variables(
        &mut f.app,
        SheetVariableMsg::NewNameTyped("2 bad".to_string()),
    );
    sheet_variables(&mut f.app, SheetVariableMsg::Added);
    assert!(
        f.app.document_state.engines[&sheet]
            .document()
            .variables
            .is_empty()
    );
    assert!(logged("Sheet variable '2 bad' was not added") > 0);
}
