use signex_types::project::ProjectData;

use super::*;

fn project() -> ProjectData {
    serde_json::from_str(r#"{"name":"Oscar","dir":"","schematic_root":null,"pcb_file":null}"#)
        .unwrap()
}

fn set_name(text: &str) -> ProjectGeneralCommand {
    ProjectGeneralCommand::SetName(text.to_string())
}

fn set_description(text: &str) -> ProjectGeneralCommand {
    ProjectGeneralCommand::SetDescription(text.to_string())
}

#[test]
fn a_run_of_edits_to_one_field_is_one_undo_step() {
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    for text in ["Osc", "Osca", "Oscar2"] {
        assert!(history.apply(&mut data, set_name(text)));
    }
    assert_eq!(data.name, "Oscar2");

    assert!(history.undo(&mut data));
    assert_eq!(data.name, "Oscar", "one undo takes the whole run back");
    assert!(!history.can_undo());
    assert!(history.redo(&mut data));
    assert_eq!(data.name, "Oscar2");
}

#[test]
fn switching_field_or_undoing_closes_the_run() {
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    history.apply(&mut data, set_name("A"));
    history.apply(&mut data, set_description("first"));
    history.apply(&mut data, set_name("B"));
    assert!(history.undo(&mut data));
    assert_eq!(data.name, "A");
    assert_eq!(data.description.as_deref(), Some("first"));
    // A new edit after an undo starts a new step and drops the redo.
    history.apply(&mut data, set_name("C"));
    assert!(!history.can_redo());
    assert!(history.undo(&mut data));
    assert_eq!(data.name, "A");
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
    assert!(!history.apply(&mut data, set_name("Oscar")));
    assert!(!history.apply(&mut data, set_description("")));
    assert!(!history.can_undo());
}

#[test]
fn undo_restores_only_the_general_fields() {
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    history.apply(&mut data, set_name("Renamed"));
    // Another writer changes a field this history does not own.
    data.variant_definitions.push("Lite".to_string());
    data.enable_git = true;
    assert!(history.undo(&mut data));
    assert_eq!(data.name, "Oscar");
    assert_eq!(data.variant_definitions, ["Lite"]);
    assert!(data.enable_git);
    assert!(history.redo(&mut data));
    assert_eq!(data.name, "Renamed");
    assert_eq!(data.variant_definitions, ["Lite"]);
}

#[test]
fn history_is_capped() {
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    for i in 0..(HISTORY_LIMIT + 10) {
        history.close_run();
        history.apply(&mut data, set_name(&format!("n{i}")));
    }
    let mut steps = 0;
    while history.undo(&mut data) {
        steps += 1;
    }
    assert_eq!(steps, HISTORY_LIMIT);
}
