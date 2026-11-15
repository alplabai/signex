//! The Component Properties edits (#2354): each builds the commands of one
//! undo step, skips footprints already as asked and refuses a locked one.

use std::collections::BTreeSet;

use signex_types::pcb::PcbBoard;
use signex_types::pcb_layer::BoardSide;
use signex_types::property::PcbProperty;
use uuid::Uuid;

use super::*;
use crate::pcb::tests::{FP_A, FP_B, board, id, snapshot};
use crate::pcb::{BoardCommand, BoardCommandError};

fn ids(list: &[&str]) -> BTreeSet<Uuid> {
    list.iter().map(|s| id(s)).collect()
}

/// Build commands from `board` with `build`, then apply them to it.
fn run(
    board: &mut PcbBoard,
    build: impl FnOnce(&PcbBoard) -> Result<Vec<BoardCommand>, ComponentRefusal>,
) {
    for command in build(board).unwrap() {
        command.apply(board).unwrap();
    }
}

fn fp(board: &PcbBoard, which: &str) -> signex_types::pcb::Footprint {
    board
        .footprints
        .iter()
        .find(|f| f.uuid == id(which))
        .unwrap()
        .clone()
}

#[test]
fn a_location_number_parses_finite_text_and_a_degree_sign() {
    assert_eq!(parse_location(LocationField::X, " 12.5 "), Ok(12.5));
    assert_eq!(parse_location(LocationField::Rotation, "90°"), Ok(90.0));
    for bad in ["", "abc", "nan", "inf", "1,5"] {
        assert!(parse_location(LocationField::Y, bad).is_err(), "{bad}");
    }
}

#[test]
fn x_moves_every_footprint_to_that_x_and_keeps_y() {
    let mut b = board();
    let both = ids(&[FP_A, FP_B]);
    let commands = location_commands(&b, &both, LocationField::X, 4.5).unwrap();
    assert_eq!(commands.len(), 2);
    run(&mut b, |_| Ok(commands));
    assert_eq!(
        (fp(&b, FP_A).position.x, fp(&b, FP_A).position.y),
        (4.5, 20.2)
    );
    assert_eq!(
        (fp(&b, FP_B).position.x, fp(&b, FP_B).position.y),
        (4.5, 7.0)
    );
    let again = location_commands(&b, &both, LocationField::X, 4.5).unwrap();
    assert!(again.is_empty(), "nothing to do makes no command");
}

#[test]
fn a_rotation_is_stored_within_one_turn() {
    let mut b = board();
    let a = ids(&[FP_A]);
    run(&mut b, |b| {
        location_commands(b, &a, LocationField::Rotation, -90.0)
    });
    assert_eq!(fp(&b, FP_A).rotation, 270.0);
    run(&mut b, |b| {
        location_commands(b, &a, LocationField::Rotation, 360.0)
    });
    assert_eq!(fp(&b, FP_A).rotation.to_bits(), 0.0f64.to_bits());
}

#[test]
fn a_side_flips_only_the_footprints_on_the_other_side() {
    let b = board();
    let both = ids(&[FP_A, FP_B]);
    let commands = side_commands(&b, &both, BoardSide::Bottom).unwrap();
    assert_eq!(
        commands,
        vec![BoardCommand::Flip {
            footprint: id(FP_A)
        }]
    );
    assert_eq!(side_of(&fp(&b, FP_B)), BoardSide::Bottom);
}

#[test]
fn a_locked_footprint_takes_only_an_unlock() {
    let mut b = board();
    let a = ids(&[FP_A]);
    run(&mut b, |b| lock_commands(b, &a, true));
    assert!(fp(&b, FP_A).locked);
    let before = snapshot(&b);
    let refused = [
        location_commands(&b, &a, LocationField::X, 1.0).err(),
        side_commands(&b, &a, BoardSide::Bottom).err(),
        text_shown_commands(&b, &a, FootprintTextField::Designator, false).err(),
    ];
    for refusal in refused {
        assert!(
            matches!(
                refusal,
                Some(ComponentRefusal::Placement(PlacementRefusal::Locked(_)))
            ),
            "{refusal:?}"
        );
    }
    assert_eq!(snapshot(&b), before);
    run(&mut b, |b| lock_commands(b, &a, false));
    assert!(!fp(&b, FP_A).locked);
}

#[test]
fn the_designator_eye_hides_its_text_and_undo_shows_it_again() {
    let mut b = board();
    let a = ids(&[FP_A]);
    assert_eq!(
        text_shown(&fp(&b, FP_A), FootprintTextField::Designator),
        Some(true)
    );
    let before = snapshot(&b);
    let commands = text_shown_commands(&b, &a, FootprintTextField::Designator, false).unwrap();
    let [command] = commands.as_slice() else {
        panic!("one command: {commands:?}");
    };
    let applied = command.clone().apply(&mut b).unwrap();
    assert!(fp(&b, FP_A).properties[0].hidden);
    assert_eq!(fp(&b, FP_A).properties[0].value, "U1");
    applied.inverse.apply(&mut b).unwrap();
    assert_eq!(snapshot(&b), before);
}

#[test]
fn a_footprint_without_the_text_refuses_the_eye() {
    let b = board();
    let refusal = text_shown_commands(
        &b,
        &ids(&[FP_A, FP_B]),
        FootprintTextField::Designator,
        false,
    );
    assert!(
        matches!(refusal, Err(ComponentRefusal::NoText { ref footprints, .. }) if footprints == &["R1"]),
        "{refusal:?}"
    );
    assert_eq!(text_shown(&fp(&b, FP_B), FootprintTextField::Value), None);
}

#[test]
fn a_parameter_value_is_set_and_the_designator_text_is_refused() {
    let mut b = board();
    b.footprints[0].properties.push(PcbProperty {
        key: "MPN".into(),
        value: "RC0603".into(),
        ..PcbProperty::default()
    });
    let a = ids(&[FP_A]);
    assert_eq!(parameter_keys(&fp(&b, FP_A)), ["MPN"]);
    run(&mut b, |b| parameter_commands(b, &a, "MPN", "RC0805"));
    assert_eq!(parameter(&fp(&b, FP_A), "MPN").unwrap().value, "RC0805");
    assert!(matches!(
        parameter_commands(&b, &a, "Reference", "U7"),
        Err(ComponentRefusal::TextParameter(_))
    ));
    assert!(matches!(
        parameter_commands(&b, &ids(&[FP_B]), "MPN", "x"),
        Err(ComponentRefusal::NoParameter { .. })
    ));
}

#[test]
fn the_property_command_keeps_its_key_and_the_shown_designator() {
    let mut b = board();
    let mut renamed = b.footprints[0].properties[0].clone();
    renamed.key = "Other".into();
    let key_change = BoardCommand::SetFootprintProperty {
        footprint: id(FP_A),
        index: 0,
        property: renamed,
    };
    assert!(matches!(
        key_change.apply(&mut b),
        Err(BoardCommandError::PropertyKeyChanged { .. })
    ));
    let mut retext = b.footprints[0].properties[0].clone();
    retext.value = "U2".into();
    let text_change = BoardCommand::SetFootprintProperty {
        footprint: id(FP_A),
        index: 0,
        property: retext,
    };
    assert!(matches!(
        text_change.apply(&mut b),
        Err(BoardCommandError::PropertyTextOwned { .. })
    ));
    let missing = BoardCommand::SetFootprintProperty {
        footprint: id(FP_A),
        index: 9,
        property: PcbProperty::default(),
    };
    assert!(matches!(
        missing.apply(&mut b),
        Err(BoardCommandError::PropertyNotFound { index: 9, .. })
    ));
}

#[test]
fn the_body_area_is_the_pad_bounds_without_a_courtyard() {
    let b = board();
    let area = body_area_mm2(&fp(&b, FP_A)).unwrap();
    assert!(area > 0.0, "{area}");
    assert_eq!(body_area_mm2(&fp(&b, FP_B)), Some(0.0));
}
