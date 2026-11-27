//! #2938: each BOM alternates edit is one undo step of the General history,
//! a refused one records nothing, and undo leaves other fields alone.

use signex_types::project::{BomAlternateProblem, ProjectData};

use super::*;

const LINE: &str = "019df45d-af5e-7a00-8c3b-dd3bf89b40ae/019df45d-af5e-7a00-8c3b-000000000001";
const ALT: &str = "019df45d-af5e-7a00-8c3b-dd3bf89b40ae/019df45d-af5e-7a00-8c3b-00000000000a";

fn project() -> ProjectData {
    serde_json::from_str(r#"{"name":"P","dir":"","schematic_root":null,"pcb_file":null}"#).unwrap()
}

#[test]
fn an_alternate_edit_is_one_undo_step_and_a_refused_one_records_nothing() {
    let mut data = project();
    let mut history = ProjectGeneralHistory::default();
    let add = ProjectBomCommand::AddAlternate {
        line: LINE.into(),
        row: ALT.into(),
    };
    assert_eq!(history.apply_bom_alternate(&mut data, &add), Ok(true));
    assert_eq!(data.bom_alternates_of(LINE), [ALT]);
    assert!(matches!(
        history.apply_bom_alternate(&mut data, &add),
        Err(BomAlternateProblem::AlreadyListed { .. })
    ));
    // Another writer's field is not put back by the undo.
    let power = signex_types::net_class::NetClass::named("Power");
    data.net_classes.push(power.clone());
    assert!(history.undo(&mut data), "the refused edit recorded no step");
    assert!(data.bom_alternates.is_empty());
    assert_eq!(data.net_classes, [power]);
    assert!(!history.can_undo());
    assert!(history.redo(&mut data));
    assert_eq!(data.bom_alternates_of(LINE), [ALT]);

    let remove = ProjectBomCommand::RemoveAlternate {
        line: LINE.into(),
        row: ALT.into(),
    };
    assert_eq!(history.apply_bom_alternate(&mut data, &remove), Ok(true));
    assert!(data.bom_alternates.is_empty());
    assert!(history.undo(&mut data));
    assert_eq!(data.bom_alternates_of(LINE), [ALT]);
}
