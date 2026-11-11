use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::*;
use crate::pcb::BoardEngine;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

// A board of `(n, reference, x, y, layer, locked)` footprints, each linked to
/// a part (unit `n + 0x1000` on the root sheet) as Update PCB leaves them.
fn board(parts: &[(u128, &str, f64, f64, &str, bool)]) -> PcbBoard {
    let footprints: Vec<_> = parts
        .iter()
        .map(|&(n, reference, x, y, layer, locked)| {
            serde_json::json!({
                "uuid": id(n), "reference": reference, "value": format!("v{n}"),
                "position": {"x": x, "y": y}, "layer": layer, "locked": locked,
                "schematic_link": {"sheet": [], "units": [id(n + 0x1000)]},
                "properties": [
                    {"key": "Reference", "value": reference},
                    {"key": "Value", "value": format!("v{n}")}
                ]
            })
        })
        .collect();
    serde_json::from_value(serde_json::json!({ "uuid": id(999), "footprints": footprints }))
        .unwrap()
}

fn top(n: u128, reference: &str, x: f64, y: f64) -> (u128, &str, f64, f64, &'static str, bool) {
    (n, reference, x, y, "TopCopper", false)
}

fn renames(plan: &ReannotatePlan) -> Vec<(String, String)> {
    plan.renames
        .iter()
        .map(|r| (r.from.clone(), r.to.clone()))
        .collect()
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
}

#[test]
fn a_designator_splits_into_prefix_and_number() {
    assert_eq!(split_designator("R12"), Some(("R", Some(12))));
    assert_eq!(split_designator("LED3"), Some(("LED", Some(3))));
    assert_eq!(split_designator("R?"), Some(("R", None)));
    assert_eq!(split_designator("TP"), Some(("TP", None)));
    assert_eq!(split_designator("12"), None);
    assert_eq!(split_designator("R1A"), None);
    assert_eq!(split_designator(""), None);
    assert_eq!(split_designator("R99999999999"), None);
}

#[test]
fn a_designator_edit_is_trimmed_and_keeps_the_value() {
    let b = board(&[top(1, "R1", 0.0, 0.0)]);
    let command = text_edit_command(&b, id(1), FootprintTextField::Designator, " R7 ")
        .unwrap()
        .unwrap();
    assert_eq!(
        command,
        BoardCommand::SetFootprintText {
            footprint: id(1),
            reference: "R7".into(),
            value: "v1".into(),
        }
    );
    let mut engine = BoardEngine::new(b, 1);
    engine.execute(command).unwrap();
    let fp = &engine.board().footprints[0];
    assert_eq!((fp.reference.as_str(), fp.value.as_str()), ("R7", "v1"));
    assert_eq!(fp.properties[0].value, "R7", "the Reference text follows");
}

#[test]
fn a_value_edit_keeps_the_designator_and_the_text_as_typed() {
    let b = board(&[top(1, "R1", 0.0, 0.0)]);
    let command = text_edit_command(&b, id(1), FootprintTextField::Value, " 10k ")
        .unwrap()
        .unwrap();
    assert_eq!(
        command,
        BoardCommand::SetFootprintText {
            footprint: id(1),
            reference: "R1".into(),
            value: " 10k ".into(),
        }
    );
}

#[test]
fn an_unchanged_edit_has_no_command() {
    let b = board(&[top(1, "R1", 0.0, 0.0)]);
    assert_eq!(
        text_edit_command(&b, id(1), FootprintTextField::Designator, "R1"),
        Ok(None)
    );
    assert_eq!(
        text_edit_command(&b, id(1), FootprintTextField::Value, "v1"),
        Ok(None)
    );
}

#[test]
fn a_locked_footprint_an_empty_designator_and_an_unknown_footprint_are_refused() {
    let b = board(&[
        top(1, "R1", 0.0, 0.0),
        (2, "R2", 0.0, 0.0, "TopCopper", true),
    ]);
    assert_eq!(
        text_edit_command(&b, id(2), FootprintTextField::Value, "x"),
        Err(TextEditRefusal::Locked("R2".into()))
    );
    assert_eq!(
        text_edit_command(&b, id(1), FootprintTextField::Designator, "  "),
        Err(TextEditRefusal::EmptyDesignator)
    );
    assert_eq!(
        text_edit_command(&b, id(9), FootprintTextField::Value, "x"),
        Err(TextEditRefusal::Command(
            BoardCommandError::FootprintNotFound(id(9))
        ))
    );
}

#[test]
fn designator_holders_are_the_other_footprints_with_it() {
    let b = board(&[
        top(1, "R1", 0.0, 0.0),
        top(2, "R1", 0.0, 0.0),
        top(3, "R2", 0.0, 0.0),
    ]);
    let holders: Vec<Uuid> = designator_holders(&b, "R1", id(1))
        .iter()
        .map(|fp| fp.uuid)
        .collect();
    assert_eq!(holders, vec![id(2)]);
    assert!(designator_holders(&b, "R2", id(3)).is_empty());
}

#[test]
fn x_then_y_numbers_left_to_right_then_top_to_bottom() {
    let b = board(&[
        top(1, "R9", 10.0, 20.0),
        top(2, "R8", 10.0, 10.0),
        top(3, "R7", 0.0, 30.0),
        top(4, "C5", 5.0, 0.0),
    ]);
    let plan = plan_reannotate(&b, ReannotateOptions::default());
    assert_eq!(
        renames(&plan),
        pairs(&[("R7", "R1"), ("C5", "C1"), ("R8", "R2"), ("R9", "R3")])
    );
    assert_eq!(plan.unchanged, 0);
}

#[test]
fn y_then_x_numbers_top_to_bottom_then_left_to_right() {
    let b = board(&[
        top(1, "R1", 10.0, 20.0),
        top(2, "R2", 10.0, 10.0),
        top(3, "R3", 0.0, 10.0),
    ]);
    let options = ReannotateOptions {
        order: ReannotateOrder::YThenX,
        ..ReannotateOptions::default()
    };
    let plan = plan_reannotate(&b, options);
    assert_eq!(
        renames(&plan),
        pairs(&[("R3", "R1"), ("R1", "R3")]),
        "R2 at (10, 10) is already second"
    );
    assert_eq!(plan.unchanged, 1);
}

#[test]
fn the_bottom_side_comes_after_the_top_unless_numbered_with_it() {
    let b = board(&[
        (1, "R1", 0.0, 0.0, "BottomCopper", false),
        top(2, "R2", 10.0, 0.0),
    ]);
    let after = plan_reannotate(&b, ReannotateOptions::default());
    assert_eq!(renames(&after), pairs(&[("R2", "R1"), ("R1", "R2")]));
    let with = ReannotateOptions {
        bottom: BottomSideOrder::WithTop,
        ..ReannotateOptions::default()
    };
    let together = plan_reannotate(&b, with);
    assert!(together.renames.is_empty());
    assert_eq!(together.unchanged, 2);
}

#[test]
fn locked_and_unnumberable_footprints_keep_their_designators_and_numbers() {
    let b = board(&[
        (1, "R1", 50.0, 0.0, "TopCopper", true),
        top(2, "R5", 0.0, 0.0),
        top(3, "R?", 10.0, 0.0),
        top(4, "12", 20.0, 0.0),
        top(5, "", 30.0, 0.0),
    ]);
    let plan = plan_reannotate(&b, ReannotateOptions::default());
    assert_eq!(
        renames(&plan),
        pairs(&[("R5", "R2"), ("R?", "R3")]),
        "the locked R1 holds number 1"
    );
    let kept: Vec<(String, KeptReason)> = plan
        .kept
        .iter()
        .map(|k| (k.reference.clone(), k.reason))
        .collect();
    assert_eq!(
        kept,
        vec![
            ("R1".to_string(), KeptReason::Locked),
            ("12".to_string(), KeptReason::NoPrefix),
            (String::new(), KeptReason::NoPrefix),
        ]
    );
}

#[test]
fn the_plan_runs_as_text_commands_and_undoes_in_one_step() {
    let b = board(&[top(1, "R2", 0.0, 0.0), top(2, "R1", 10.0, 0.0)]);
    let plan = plan_reannotate(&b, ReannotateOptions::default());
    let commands = plan.commands(&b).unwrap();
    let mut engine = BoardEngine::new(b.clone(), 1);
    engine.execute_batch(commands).unwrap();
    let refs: Vec<&str> = engine
        .board()
        .footprints
        .iter()
        .map(|fp| fp.reference.as_str())
        .collect();
    assert_eq!(refs, vec!["R1", "R2"]);
    assert_eq!(engine.board().footprints[0].value, "v1");
    engine.undo().unwrap();
    assert_eq!(engine.board().footprints, b.footprints);
}

#[test]
fn a_plan_for_a_footprint_that_is_gone_is_refused() {
    let b = board(&[top(1, "R2", 0.0, 0.0)]);
    let plan = plan_reannotate(&b, ReannotateOptions::default());
    let empty = board(&[]);
    assert_eq!(
        plan.commands(&empty),
        Err(BoardCommandError::FootprintNotFound(id(1)))
    );
}

#[test]
fn surrounding_spaces_are_trimmed_and_other_digits_are_not_a_number() {
    assert_eq!(split_designator(" R1 "), Some(("R", Some(1))));
    assert_eq!(split_designator("R01"), Some(("R", Some(1))));
    assert_eq!(split_designator("R\u{ff11}"), None, "a full-width digit");
    let b = board(&[top(1, " R1", 0.0, 0.0), top(2, "R\u{ff11}", 10.0, 0.0)]);
    let plan = plan_reannotate(&b, ReannotateOptions::default());
    assert_eq!(renames(&plan), pairs(&[(" R1", "R1")]));
    assert_eq!(plan.kept.len(), 1);
    assert_eq!(plan.kept[0].reason, KeptReason::NoPrefix);
}

#[test]
fn unlinked_footprints_keep_their_designators_and_numbers() {
    let mut b = board(&[
        top(1, "R1", 50.0, 0.0),
        top(2, "R5", 0.0, 0.0),
        top(3, "R9", 10.0, 0.0),
    ]);
    b.footprints[0].schematic_link = None;
    let plan = plan_reannotate(&b, ReannotateOptions::default());
    assert_eq!(
        renames(&plan),
        pairs(&[("R5", "R2"), ("R9", "R3")]),
        "the unlinked R1 keeps its number"
    );
    assert_eq!(plan.kept[0].reason, KeptReason::Unlinked);
}
