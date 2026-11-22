//! Rows m29 (General), m30 (Parameters) and m31 (Sheets) of the Project tab.

use signex_app::app::{
    DocumentSetupMsg, Message, NewProjectMsg, ProjectGeneralFacts, ProjectSection, ProjectTabMsg,
    SetupKind,
};
use signex_types::designator::DesignatorStandard;
use signex_types::project::ListMove;

use super::support::{Bench, bench};

const THREE_SHEETS: &[(&str, &str)] = &[("root.snxsch", ""), ("b.snxsch", ""), ("c.snxsch", "")];

fn on(section: ProjectSection) -> Bench {
    let mut b = bench(THREE_SHEETS);
    b.open_section(section);
    b
}

#[test]
fn m29_the_section_shows_the_projects_files() {
    let b = on(ProjectSection::General);
    let facts = ProjectGeneralFacts::of(&b.app.document_state.projects[0]);
    assert_eq!(facts.schematic_root.as_deref(), Some("root.snxsch"));
    assert_eq!(facts.pcb_file, None);
    assert_eq!(facts.library_count, 0);
}

#[test]
fn m29_a_description_edit_changes_the_project_and_dirties_it() {
    let mut b = on(ProjectSection::General);
    let project_id = b.id;
    b.tab(ProjectTabMsg::DescriptionEdited {
        project_id,
        text: "Buck converter".into(),
    });
    assert_eq!(b.data().description.as_deref(), Some("Buck converter"));
    assert!(b.dirty());
    assert_eq!(b.on_disk().description, None, "nothing until Save");
}

#[test]
fn m29_undo_takes_the_description_back_and_clears_the_dirty_mark() {
    let mut b = on(ProjectSection::General);
    let project_id = b.id;
    b.tab(ProjectTabMsg::DescriptionEdited {
        project_id,
        text: "Buck converter".into(),
    });
    b.undo();
    assert_eq!(b.data().description, None);
    assert!(!b.dirty());
    b.redo();
    assert_eq!(b.data().description.as_deref(), Some("Buck converter"));
    assert!(b.dirty());
}

#[test]
fn m29_the_designator_standard_pick_is_stored_and_undone() {
    let mut b = on(ProjectSection::General);
    let project_id = b.id;
    b.tab(ProjectTabMsg::DesignatorStandardPicked {
        project_id,
        standard: Some(DesignatorStandard::Iec81346_2),
    });
    assert_eq!(
        b.data().designator_standard(),
        Some(DesignatorStandard::Iec81346_2)
    );
    assert!(b.dirty());
    b.undo();
    assert_eq!(b.data().designator_standard(), None);
    assert!(!b.dirty());
}

#[test]
fn m29_rename_opens_the_project_rename_dialog_without_dirtying() {
    let mut b = on(ProjectSection::General);
    let project_id = b.id;
    b.tab(ProjectTabMsg::RenameRequested { project_id });
    let dialog = b.app.ui_state.rename_dialog.as_ref().expect("the dialog");
    assert!(dialog.is_project_rename);
    assert_eq!(dialog.target_path, b.prj);
    assert!(!b.dirty());
}

fn add_parameter(b: &mut Bench, name: &str) {
    let project_id = b.id;
    b.tab(ProjectTabMsg::NewParameterNameEdited {
        project_id,
        text: name.to_string(),
    });
    b.tab(ProjectTabMsg::ParameterAdded { project_id });
}

fn set_parameter(b: &mut Bench, name: &str, value: &str) {
    let project_id = b.id;
    b.tab(ProjectTabMsg::ParameterEdited {
        project_id,
        name: name.to_string(),
        value: value.to_string(),
    });
}

fn rename_parameter(b: &mut Bench, from: &str, text: &str) {
    let project_id = b.id;
    b.tab(ProjectTabMsg::ParameterRenameEdited {
        project_id,
        from: from.to_string(),
        text: text.to_string(),
    });
    b.tab(ProjectTabMsg::ParameterRenameSubmitted { project_id });
}

#[test]
fn m30_add_makes_an_empty_variable_and_clears_the_name_box() {
    let mut b = on(ProjectSection::Parameters);
    add_parameter(&mut b, "VOUT");
    assert_eq!(
        b.data().parameters.get("VOUT").map(String::as_str),
        Some("")
    );
    assert_eq!(b.tab_state().new_parameter_name, "");
    assert!(b.dirty());
}

#[test]
fn m30_undo_removes_the_added_variable() {
    let mut b = on(ProjectSection::Parameters);
    add_parameter(&mut b, "VOUT");
    b.undo();
    assert!(!b.data().parameters.contains_key("VOUT"));
    assert!(!b.dirty());
}

#[test]
fn m30_a_name_that_is_no_variable_is_refused_and_stays_typed() {
    let mut b = on(ProjectSection::Parameters);
    add_parameter(&mut b, "2 bad");
    assert!(b.data().parameters.is_empty());
    assert_eq!(b.tab_state().new_parameter_name, "2 bad");
    assert!(!b.dirty());
}

#[test]
fn m30_a_value_edit_is_stored_and_undone() {
    let mut b = on(ProjectSection::Parameters);
    add_parameter(&mut b, "VOUT");
    set_parameter(&mut b, "VOUT", "3.3");
    assert_eq!(
        b.data().parameters.get("VOUT").map(String::as_str),
        Some("3.3")
    );
    b.undo();
    assert_eq!(
        b.data().parameters.get("VOUT").map(String::as_str),
        Some("")
    );
}

#[test]
fn m30_a_title_block_value_is_stored_under_its_key() {
    let mut b = on(ProjectSection::Parameters);
    let key = signex_app::panels::TitleBlockField::Revision.key();
    set_parameter(&mut b, key, "B");
    assert_eq!(b.data().parameters.get(key).map(String::as_str), Some("B"));
}

#[test]
fn m30_rename_moves_the_variable_and_keeps_its_value_as_one_undo_step() {
    let mut b = on(ProjectSection::Parameters);
    add_parameter(&mut b, "VOUT");
    set_parameter(&mut b, "VOUT", "3.3");
    rename_parameter(&mut b, "VOUT", "VREF");
    assert_eq!(
        b.data().parameters.get("VREF").map(String::as_str),
        Some("3.3")
    );
    assert!(!b.data().parameters.contains_key("VOUT"));
    assert!(b.tab_state().parameter_rename.is_none());
    b.undo();
    assert_eq!(
        b.data().parameters.get("VOUT").map(String::as_str),
        Some("3.3")
    );
    assert!(!b.data().parameters.contains_key("VREF"));
}

#[test]
fn m30_rename_to_a_taken_name_is_refused_and_stays_in_its_box() {
    let mut b = on(ProjectSection::Parameters);
    add_parameter(&mut b, "VOUT");
    add_parameter(&mut b, "VREF");
    rename_parameter(&mut b, "VOUT", "VREF");
    assert!(b.data().parameters.contains_key("VOUT"));
    assert!(b.tab_state().parameter_rename_refused);
    assert_eq!(
        b.tab_state().parameter_rename,
        Some(("VOUT".to_string(), "VREF".to_string()))
    );
}

#[test]
fn m30_rename_to_a_title_block_key_is_refused() {
    let mut b = on(ProjectSection::Parameters);
    add_parameter(&mut b, "VOUT");
    let key = signex_app::panels::TitleBlockField::Title.key();
    rename_parameter(&mut b, "VOUT", key);
    assert!(b.data().parameters.contains_key("VOUT"));
    assert!(!b.data().parameters.contains_key(key));
    assert!(b.tab_state().parameter_rename_refused);
}

#[test]
fn m30_remove_drops_the_variable_and_undo_restores_it() {
    let mut b = on(ProjectSection::Parameters);
    add_parameter(&mut b, "VOUT");
    set_parameter(&mut b, "VOUT", "3.3");
    let project_id = b.id;
    b.tab(ProjectTabMsg::ParameterRemoved {
        project_id,
        name: "VOUT".into(),
    });
    assert!(!b.data().parameters.contains_key("VOUT"));
    b.undo();
    assert_eq!(
        b.data().parameters.get("VOUT").map(String::as_str),
        Some("3.3")
    );
}

#[test]
fn m30_the_filter_is_view_state_and_does_not_dirty_the_project() {
    let mut b = on(ProjectSection::Parameters);
    let project_id = b.id;
    b.tab(ProjectTabMsg::ParameterFilterEdited {
        project_id,
        text: "REV".into(),
    });
    assert_eq!(b.tab_state().parameter_filter, "REV");
    assert!(!b.dirty());
}

fn move_sheet(b: &mut Bench, filename: &str, step: ListMove) {
    let project_id = b.id;
    b.tab(ProjectTabMsg::SheetMoved {
        project_id,
        filename: filename.to_string(),
        step,
    });
}

#[test]
fn m31_move_down_reorders_the_pages_and_dirties_the_project() {
    let mut b = on(ProjectSection::Sheets);
    move_sheet(&mut b, "b.snxsch", ListMove::Down);
    assert_eq!(b.listed(), ["root.snxsch", "c.snxsch", "b.snxsch"]);
    assert!(b.dirty());
}

#[test]
fn m31_move_up_reorders_and_undo_puts_the_order_back() {
    let mut b = on(ProjectSection::Sheets);
    move_sheet(&mut b, "c.snxsch", ListMove::Up);
    assert_eq!(b.listed(), ["root.snxsch", "c.snxsch", "b.snxsch"]);
    b.undo();
    assert_eq!(b.listed(), ["root.snxsch", "b.snxsch", "c.snxsch"]);
    assert!(!b.dirty());
}

#[test]
fn m31_moving_past_the_end_changes_nothing_and_stays_clean() {
    let mut b = on(ProjectSection::Sheets);
    move_sheet(&mut b, "root.snxsch", ListMove::Up);
    move_sheet(&mut b, "c.snxsch", ListMove::Down);
    assert_eq!(b.listed(), ["root.snxsch", "b.snxsch", "c.snxsch"]);
    assert!(!b.dirty());
}

#[test]
fn m31_the_new_sheet_size_is_stored_in_the_project_and_undone() {
    let mut b = on(ProjectSection::Sheets);
    let project_id = b.id;
    b.tab(ProjectTabMsg::SetupEdited {
        project_id,
        msg: NewProjectMsg::SheetSizePicked("A2".into()),
    });
    let sheet = b.data().project_defaults.sheet.as_ref().expect("stored");
    assert_eq!(sheet.paper_size, "A2");
    assert!(b.dirty());
    b.undo();
    assert!(b.data().project_defaults.sheet.is_none());
    assert!(!b.dirty());
}

#[test]
fn m31_add_schematic_opens_the_new_schematic_dialog() {
    let mut b = on(ProjectSection::Sheets);
    let project_id = b.id;
    b.tab(ProjectTabMsg::AddSchematicRequested { project_id });
    let state = b.app.ui_state.document_setup.as_ref().expect("the dialog");
    assert_eq!(state.kind, SetupKind::Sheet);
    assert_eq!(state.project, b.id);
    b.send(Message::from(DocumentSetupMsg::CancelRequested));
    assert_eq!(b.listed().len(), 3);
}
