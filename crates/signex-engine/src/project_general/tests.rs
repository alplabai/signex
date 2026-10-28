use signex_types::project::ProjectData;

use super::*;

fn project() -> ProjectData {
    serde_json::from_str(r#"{"name":"Oscar","dir":"","schematic_root":null,"pcb_file":null}"#)
        .unwrap()
}

fn set_description(text: &str) -> ProjectGeneralCommand {
    ProjectGeneralCommand::SetDescription(text.to_string())
}

#[test]
fn a_run_of_edits_is_one_undo_step() {
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    for text in ["M", "Mo", "Motor"] {
        assert!(history.apply(&mut data, set_description(text)));
    }
    assert!(history.undo(&mut data));
    assert_eq!(data.description, None, "one undo takes the whole run back");
    assert!(!history.can_undo());
    assert!(history.redo(&mut data));
    assert_eq!(data.description.as_deref(), Some("Motor"));
}

#[test]
fn closing_the_run_or_undoing_starts_a_new_step() {
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    history.apply(&mut data, set_description("a"));
    history.close_run();
    history.apply(&mut data, set_description("ab"));
    assert!(history.undo(&mut data));
    assert_eq!(data.description.as_deref(), Some("a"));
    // A new edit after an undo starts a new step and drops the redo.
    history.apply(&mut data, set_description("ac"));
    assert!(!history.can_redo());
    assert!(history.undo(&mut data));
    assert_eq!(data.description.as_deref(), Some("a"));
}

#[test]
fn an_empty_description_is_stored_as_none() {
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    history.apply(&mut data, set_description("x"));
    history.apply(&mut data, set_description(""));
    assert_eq!(data.description, None);
}

#[test]
fn an_edit_that_changes_nothing_records_nothing() {
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    assert!(!history.apply(&mut data, set_description("")));
    assert!(!history.can_undo());
}

#[test]
fn undo_restores_only_the_general_fields() {
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    history.apply(&mut data, set_description("mine"));
    // Other writers change fields this history does not own.
    data.name = "Renamed".to_string();
    data.variant_definitions.push("Lite".to_string());
    assert!(history.undo(&mut data));
    assert_eq!(data.description, None);
    assert_eq!(data.name, "Renamed");
    assert_eq!(data.variant_definitions, ["Lite"]);
    assert!(history.redo(&mut data));
    assert_eq!(data.description.as_deref(), Some("mine"));
    assert_eq!(data.name, "Renamed");
}

#[test]
fn history_is_capped() {
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    for i in 0..(HISTORY_LIMIT + 10) {
        history.close_run();
        history.apply(&mut data, set_description(&format!("d{i}")));
    }
    let mut steps = 0;
    while history.undo(&mut data) {
        steps += 1;
    }
    assert_eq!(steps, HISTORY_LIMIT);
}

fn set_parameter(name: &str, value: &str) -> ProjectGeneralCommand {
    ProjectGeneralCommand::SetParameter {
        name: name.to_string(),
        value: value.to_string(),
    }
}

#[test]
fn parameters_are_set_cleared_removed_and_undone() {
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    assert!(history.apply(&mut data, set_parameter("rev", "B")));
    history.close_run();
    assert!(history.apply(&mut data, set_parameter("Voltage", "")));
    assert_eq!(data.parameters.get("Voltage").map(String::as_str), Some(""));
    history.close_run();
    // Clearing a title-block key drops it; a user key stays until removed.
    assert!(history.apply(&mut data, set_parameter("rev", "")));
    assert!(!data.parameters.contains_key("rev"));
    history.close_run();
    assert!(history.apply(
        &mut data,
        ProjectGeneralCommand::RemoveParameter("Voltage".into())
    ));
    assert!(data.parameters.is_empty());
    assert!(!history.apply(
        &mut data,
        ProjectGeneralCommand::RemoveParameter("Voltage".into())
    ));
    assert!(history.undo(&mut data));
    assert!(data.parameters.contains_key("Voltage"));
    assert!(history.undo(&mut data));
    assert_eq!(data.parameters.get("rev").map(String::as_str), Some("B"));
    assert!(history.redo(&mut data));
    assert!(!data.parameters.contains_key("rev"));
}
