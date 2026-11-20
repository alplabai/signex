//! The Parameters section (Graphite mockup m30): the filter, the folded
//! groups and renaming a variable in place, driven through `Signex::update`.

use std::collections::BTreeMap;

use super::rename::{ParameterNameProblem, RenameNote, checked_rename, rename_note};
use super::{ParameterGroup, visible_parameters};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, ProjectSection, ProjectTabMsg, Signex};
use crate::panels::TitleBlockField;

fn app() -> (Signex, ProjectId) {
    let dir = std::env::temp_dir().join(format!("signex-prj-params-{}", uuid::Uuid::now_v7()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["a.snxsch"]);
    app.open_project_tab(0);
    let id = app.document_state.projects[0].id;
    (app, id)
}

fn send(app: &mut Signex, msg: ProjectTabMsg) {
    let _ = app.update(Message::Project(ProjectMsg::Tab(msg)));
}

fn dirty(app: &Signex) -> bool {
    let path = &app.document_state.projects[0].path;
    app.document_state.is_document_dirty(path)
}

fn parameters(app: &Signex) -> &BTreeMap<String, String> {
    &app.document_state.projects[0].data.parameters
}

fn undo(app: &mut Signex, id: ProjectId) -> bool {
    app.step_project_tab_history(id, true)
}

/// How many General undo steps the tab holds, counted on copies.
fn undo_steps(app: &Signex, id: ProjectId) -> usize {
    let mut history = app.document_state.project_tabs[&id].general_history.clone();
    let mut data = app.document_state.projects[0].data.clone();
    let mut steps = 0;
    while history.undo(&mut data) {
        steps += 1;
    }
    steps
}

/// Add the variable `name` with `value` through the tab: two undo steps.
fn add_variable(app: &mut Signex, id: ProjectId, name: &str, value: &str) {
    send(
        app,
        ProjectTabMsg::NewParameterNameEdited {
            project_id: id,
            text: name.into(),
        },
    );
    send(app, ProjectTabMsg::ParameterAdded { project_id: id });
    send(
        app,
        ProjectTabMsg::ParameterEdited {
            project_id: id,
            name: name.into(),
            value: value.into(),
        },
    );
}

fn rename(app: &mut Signex, id: ProjectId, from: &str, text: &str) {
    send(
        app,
        ProjectTabMsg::ParameterRenameEdited {
            project_id: id,
            from: from.into(),
            text: text.into(),
        },
    );
    send(
        app,
        ProjectTabMsg::ParameterRenameSubmitted { project_id: id },
    );
}

fn english(field: TitleBlockField) -> String {
    field.label().to_string()
}

#[test]
fn the_filter_matches_the_shown_name_or_the_value_ignoring_case() {
    let parameters = BTreeMap::from([
        ("company".to_string(), "Acme Ltd".to_string()),
        ("rev".to_string(), "B".to_string()),
        ("VIN".to_string(), "12V".to_string()),
        ("FSW".to_string(), "500kHz".to_string()),
    ]);

    let all = visible_parameters(&parameters, "", english);
    assert_eq!(all.title_block, TitleBlockField::ALL.to_vec());
    assert_eq!(
        all.variables,
        ["FSW", "VIN"],
        "title-block keys are not variables"
    );
    assert_eq!(visible_parameters(&parameters, "   ", english), all);

    // The shown name of a title-block field, not its key.
    let by_name = visible_parameters(&parameters, "document no", english);
    assert_eq!(by_name.title_block, [TitleBlockField::DocumentNumber]);
    assert!(by_name.variables.is_empty());

    // A value, in either group, whatever the case.
    let by_value = visible_parameters(&parameters, "ACME", english);
    assert_eq!(by_value.title_block, [TitleBlockField::Company]);
    let by_value = visible_parameters(&parameters, "khz", english);
    assert!(by_value.title_block.is_empty());
    assert_eq!(by_value.variables, ["FSW"]);

    // A variable's key.
    let by_key = visible_parameters(&parameters, "vi", english);
    assert_eq!(by_key.variables, ["VIN"]);
}

#[test]
fn a_rename_is_checked_before_the_engine_sees_it() {
    let parameters = BTreeMap::from([
        ("VIN".to_string(), "12V".to_string()),
        ("VOUT".to_string(), "3V3".to_string()),
    ]);
    let check = |text: &str| checked_rename(&parameters, "VIN", text);
    assert_eq!(check("VSUPPLY"), Ok(Some("VSUPPLY".to_string())));
    assert_eq!(check("  VSUPPLY "), Ok(Some("VSUPPLY".to_string())));
    assert_eq!(check("VIN"), Ok(None), "its own name is nothing to do");
    assert_eq!(
        check("vout"),
        Ok(Some("vout".to_string())),
        "case-sensitive"
    );
    assert_eq!(check(""), Err(ParameterNameProblem::Empty));
    assert_eq!(check("   "), Err(ParameterNameProblem::Empty));
    assert_eq!(check("V\nIN"), Err(ParameterNameProblem::Invalid));
    assert_eq!(check("V\tIN"), Err(ParameterNameProblem::Invalid));
    // #1987: a parameter is a variable, named like the Add row requires.
    assert_eq!(check("V IN"), Err(ParameterNameProblem::Invalid));
    assert_eq!(check("1V"), Err(ParameterNameProblem::Invalid));
    assert_eq!(check("V-IN"), Err(ParameterNameProblem::Invalid));
    assert_eq!(check("_V1"), Ok(Some("_V1".to_string())));
    assert_eq!(check("company"), Err(ParameterNameProblem::TitleBlock));
    assert_eq!(check("rev"), Err(ParameterNameProblem::TitleBlock));
    assert_eq!(check("VOUT"), Err(ParameterNameProblem::Taken));
}

/// Renaming moves only the project key, so `${VIN}` on a sheet and a sheet
/// value named VIN stop resolving to it: a name that Enter would apply is
/// warned about before Enter; a refusal shows only after Enter.
#[test]
fn an_accepted_rename_draft_warns_that_references_keep_the_old_name() {
    let parameters = BTreeMap::from([
        ("VIN".to_string(), "12V".to_string()),
        ("VOUT".to_string(), "3V3".to_string()),
    ]);
    let note = |text: &str, refused: bool| rename_note(&parameters, "VIN", text, refused);
    assert_eq!(
        note("VSUPPLY", false),
        Some(RenameNote::ReferencesKeepOldName)
    );
    assert_eq!(
        note("VSUPPLY", true),
        Some(RenameNote::ReferencesKeepOldName)
    );
    assert_eq!(note("VIN", false), None, "its own name renames nothing");
    assert_eq!(note(" VIN ", false), None);
    assert_eq!(note("VOUT", false), None, "not refused until Enter");
    assert_eq!(
        note("VOUT", true),
        Some(RenameNote::Refused(ParameterNameProblem::Taken))
    );
}

#[test]
fn a_rename_through_the_tab_is_one_undo_step_and_dirties_the_project() {
    let (mut app, id) = app();
    add_variable(&mut app, id, "VIN", "12V");
    rename(&mut app, id, "VIN", "VSUPPLY");
    assert_eq!(
        parameters(&app).get("VSUPPLY").map(String::as_str),
        Some("12V")
    );
    assert!(!parameters(&app).contains_key("VIN"));
    assert!(dirty(&app));
    let tab = &app.document_state.project_tabs[&id];
    assert_eq!(tab.parameter_rename, None, "the draft is spent");
    assert!(!tab.parameter_rename_refused);

    assert!(undo(&mut app, id), "one step takes the rename back");
    assert_eq!(parameters(&app).get("VIN").map(String::as_str), Some("12V"));
    assert!(!parameters(&app).contains_key("VSUPPLY"));
    // Then the value, then the add: back to the file's content.
    assert!(undo(&mut app, id));
    assert!(undo(&mut app, id));
    assert!(parameters(&app).is_empty());
    assert!(!dirty(&app));
}

#[test]
fn a_refused_rename_keeps_the_data_and_the_draft_and_records_nothing() {
    let (mut app, id) = app();
    add_variable(&mut app, id, "VIN", "12V");
    add_variable(&mut app, id, "VOUT", "3V3");
    let before = parameters(&app).clone();
    let steps_before = undo_steps(&app, id);
    for text in ["", "  ", "V\nIN", "company", "VOUT"] {
        rename(&mut app, id, "VIN", text);
        assert_eq!(parameters(&app), &before, "{text:?} changes nothing");
        let tab = &app.document_state.project_tabs[&id];
        assert!(tab.parameter_rename_refused, "{text:?} is refused");
        assert_eq!(
            tab.parameter_rename,
            Some(("VIN".to_string(), text.to_string())),
            "the refused name stays in its box"
        );
    }
    assert_eq!(undo_steps(&app, id), steps_before, "no step was recorded");

    // The next keystroke clears the refusal.
    send(
        &mut app,
        ProjectTabMsg::ParameterRenameEdited {
            project_id: id,
            from: "VIN".into(),
            text: "VI".into(),
        },
    );
    assert!(!app.document_state.project_tabs[&id].parameter_rename_refused);
}

#[test]
fn the_rename_draft_is_cleared_on_a_section_switch_and_on_undo() {
    let (mut app, id) = app();
    add_variable(&mut app, id, "VIN", "12V");
    rename(&mut app, id, "VIN", "VOUT ");
    send(
        &mut app,
        ProjectTabMsg::ParameterRenameEdited {
            project_id: id,
            from: "VOUT".into(),
            text: "".into(),
        },
    );
    send(
        &mut app,
        ProjectTabMsg::ParameterRenameSubmitted { project_id: id },
    );
    assert!(app.document_state.project_tabs[&id].parameter_rename_refused);
    send(
        &mut app,
        ProjectTabMsg::SectionSelected {
            project_id: id,
            section: ProjectSection::Sheets,
        },
    );
    let tab = &app.document_state.project_tabs[&id];
    assert_eq!(tab.parameter_rename, None);
    assert!(!tab.parameter_rename_refused);

    send(
        &mut app,
        ProjectTabMsg::ParameterRenameEdited {
            project_id: id,
            from: "VOUT".into(),
            text: "VO".into(),
        },
    );
    assert!(undo(&mut app, id));
    assert_eq!(app.document_state.project_tabs[&id].parameter_rename, None);
}

#[test]
fn the_filter_and_the_folded_groups_are_tab_state_only() {
    let (mut app, id) = app();
    add_variable(&mut app, id, "VIN", "12V");
    let dirty_before = dirty(&app);
    send(
        &mut app,
        ProjectTabMsg::ParameterFilterEdited {
            project_id: id,
            text: "vin".into(),
        },
    );
    send(
        &mut app,
        ProjectTabMsg::ParameterGroupToggled {
            project_id: id,
            group: ParameterGroup::TitleBlock,
        },
    );
    let tab = &app.document_state.project_tabs[&id];
    assert_eq!(tab.parameter_filter, "vin");
    assert!(!tab.is_parameter_group_open(ParameterGroup::TitleBlock));
    assert!(tab.is_parameter_group_open(ParameterGroup::Variables));
    assert_eq!(dirty(&app), dirty_before, "no document edit");

    send(
        &mut app,
        ProjectTabMsg::ParameterGroupToggled {
            project_id: id,
            group: ParameterGroup::TitleBlock,
        },
    );
    let tab = &app.document_state.project_tabs[&id];
    assert!(tab.is_parameter_group_open(ParameterGroup::TitleBlock));
}

#[test]
fn a_late_message_for_a_closed_project_leaves_no_tab_state() {
    let (mut app, _id) = app();
    let gone = app.document_state.mint_project_id();
    for msg in [
        ProjectTabMsg::ParameterFilterEdited {
            project_id: gone,
            text: "x".into(),
        },
        ProjectTabMsg::ParameterGroupToggled {
            project_id: gone,
            group: ParameterGroup::Variables,
        },
        ProjectTabMsg::ParameterRenameEdited {
            project_id: gone,
            from: "VIN".into(),
            text: "VOUT".into(),
        },
        ProjectTabMsg::ParameterRenameSubmitted { project_id: gone },
    ] {
        send(&mut app, msg);
    }
    assert!(!app.document_state.project_tabs.contains_key(&gone));
}

#[test]
fn the_section_renders_with_a_refused_rename_a_filter_and_a_folded_group() {
    let (mut app, id) = app();
    add_variable(&mut app, id, "VIN", "12V");
    add_variable(&mut app, id, "VOUT", "");
    send(
        &mut app,
        ProjectTabMsg::SectionSelected {
            project_id: id,
            section: ProjectSection::Parameters,
        },
    );
    let _ = app.view_project_tab(id);
    send(
        &mut app,
        ProjectTabMsg::ParameterRenameEdited {
            project_id: id,
            from: "VIN".into(),
            text: "VSUPPLY".into(),
        },
    );
    let _ = app.view_project_tab(id);
    rename(&mut app, id, "VIN", "VOUT");
    let _ = app.view_project_tab(id);
    send(
        &mut app,
        ProjectTabMsg::ParameterFilterEdited {
            project_id: id,
            text: "v".into(),
        },
    );
    send(
        &mut app,
        ProjectTabMsg::ParameterGroupToggled {
            project_id: id,
            group: ParameterGroup::Variables,
        },
    );
    let _ = app.view_project_tab(id);
}
