//! Break Wire and the merge of collinear wire segments (#1254).

use crate::rubber_band_tests::{add_resistor, add_wire, net_shape};
use crate::*;
use signex_types::schematic::{Junction, Label, LabelType, Point, SelectedKind, Wire};
use uuid::Uuid;

fn wire_item(uuid: Uuid) -> SelectedItem {
    SelectedItem::new(uuid, SelectedKind::Wire)
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn new_wire(a: (f64, f64), b: (f64, f64)) -> Wire {
    Wire {
        uuid: Uuid::new_v4(),
        start: pt(a.0, a.1),
        end: pt(b.0, b.1),
        ..Default::default()
    }
}

fn place(engine: &mut Engine, wire: Wire) {
    engine
        .execute(Command::PlaceWireSegment { wire })
        .expect("place");
}

fn spans(engine: &Engine) -> Vec<((f64, f64), (f64, f64))> {
    engine
        .document()
        .wires
        .iter()
        .map(|w| ((w.start.x, w.start.y), (w.end.x, w.end.y)))
        .collect()
}

/// R1 at (30, 50) has pin 2 at (35, 50); R2 at (60, 50) has pin 1 at (55, 50).
fn two_resistors() -> SchematicSheet {
    let mut sheet = SchematicSheet::default();
    add_resistor(&mut sheet, "R1", pt(30.0, 50.0));
    add_resistor(&mut sheet, "R2", pt(60.0, 50.0));
    sheet
}

/// A wire from R1's pin 2 to (45, 50), ready for a collinear neighbour.
fn engine_with_first_leg() -> (Engine, Uuid) {
    let mut sheet = two_resistors();
    let first = add_wire(&mut sheet, (35.0, 50.0), (45.0, 50.0));
    (Engine::new(sheet).expect("engine"), first)
}

// --- Break Wire ------------------------------------------------------------

fn engine_with_one_wire() -> (Engine, Uuid) {
    let mut sheet = SchematicSheet::default();
    let wire = add_wire(&mut sheet, (0.0, 10.0), (20.0, 10.0));
    (Engine::new(sheet).expect("engine"), wire)
}

#[test]
fn break_wire_splits_one_wire_in_two_at_the_point() {
    let (mut engine, wire) = engine_with_one_wire();
    let result = engine
        .execute(Command::BreakWire {
            wire,
            at: pt(8.0, 10.0),
        })
        .unwrap();
    assert!(result.changed);

    assert_eq!(
        spans(&engine),
        [((0.0, 10.0), (8.0, 10.0)), ((8.0, 10.0), (20.0, 10.0))]
    );
    assert_eq!(
        engine.document().wires[0].uuid,
        wire,
        "the first keeps its id"
    );
    assert_ne!(engine.document().wires[1].uuid, wire);
}

#[test]
fn break_wire_cuts_at_the_nearest_point_of_an_axis_aligned_wire() {
    let (mut engine, wire) = engine_with_one_wire();
    assert_eq!(engine.wire_to_break_at(pt(8.0, 10.1)), Some(wire));
    assert_eq!(engine.wire_to_break_at(pt(8.0, 11.0)), None);
    engine
        .execute(Command::BreakWire {
            wire,
            at: pt(8.0, 10.1),
        })
        .unwrap();
    assert_eq!(
        spans(&engine),
        [((0.0, 10.0), (8.0, 10.0)), ((8.0, 10.0), (20.0, 10.0))],
        "the cut sits on the wire's own line"
    );
}

#[test]
fn break_wire_keeps_the_halves_apart_and_is_one_undo_step() {
    let (mut engine, wire) = engine_with_one_wire();
    engine
        .execute(Command::BreakWire {
            wire,
            at: pt(8.0, 10.0),
        })
        .unwrap();
    assert_eq!(
        engine.document().wires.len(),
        2,
        "no merge pass re-joins them"
    );

    assert!(engine.undo().is_ok_and(|undone| undone.is_some()));
    assert_eq!(spans(&engine), [((0.0, 10.0), (20.0, 10.0))]);
}

#[test]
fn break_wire_does_not_change_the_nets() {
    let mut sheet = two_resistors();
    let wire = add_wire(&mut sheet, (35.0, 50.0), (55.0, 50.0));
    let before = net_shape(&sheet);
    let mut engine = Engine::new(sheet).expect("engine");
    engine
        .execute(Command::BreakWire {
            wire,
            at: pt(45.0, 50.0),
        })
        .unwrap();
    assert_eq!(engine.document().wires.len(), 2);
    assert_eq!(net_shape(engine.document()), before);
}

#[test]
fn break_wire_off_the_wire_at_an_end_or_on_a_missing_wire_does_nothing() {
    let (mut engine, wire) = engine_with_one_wire();
    for (name, command) in [
        (
            "off the wire",
            Command::BreakWire {
                wire,
                at: pt(8.0, 12.0),
            },
        ),
        (
            "beyond the end",
            Command::BreakWire {
                wire,
                at: pt(25.0, 10.0),
            },
        ),
        (
            "at the start",
            Command::BreakWire {
                wire,
                at: pt(0.0, 10.0),
            },
        ),
        (
            "at the end",
            Command::BreakWire {
                wire,
                at: pt(20.0, 10.0),
            },
        ),
        (
            "a missing wire",
            Command::BreakWire {
                wire: Uuid::new_v4(),
                at: pt(8.0, 10.0),
            },
        ),
    ] {
        let result = engine.execute(command).unwrap();
        assert!(!result.changed, "{name}");
    }
    assert_eq!(spans(&engine), [((0.0, 10.0), (20.0, 10.0))]);
    assert!(engine.undo().is_ok_and(|undone| undone.is_none()));
}

#[test]
fn break_wire_splits_a_diagonal_wire_and_keeps_its_width() {
    let mut sheet = SchematicSheet::default();
    let wire = add_wire(&mut sheet, (0.0, 0.0), (10.0, 10.0));
    sheet.wires[0].stroke_width = 0.3;
    let mut engine = Engine::new(sheet).expect("engine");
    engine
        .execute(Command::BreakWire {
            wire,
            at: pt(4.0, 4.0),
        })
        .unwrap();
    assert_eq!(
        spans(&engine),
        [((0.0, 0.0), (4.0, 4.0)), ((4.0, 4.0), (10.0, 10.0))]
    );
    assert!(
        engine
            .document()
            .wires
            .iter()
            .all(|w| w.stroke_width == 0.3)
    );
}

#[test]
fn break_wire_on_a_locked_wire_is_refused() {
    let (mut engine, wire) = engine_with_one_wire();
    engine
        .execute(Command::SetItemLocked {
            item: wire_item(wire),
            locked: true,
        })
        .unwrap();
    let refused = engine.execute(Command::BreakWire {
        wire,
        at: pt(8.0, 10.0),
    });
    assert!(matches!(refused, Err(EngineError::ItemLocked { .. })));
    assert_eq!(engine.document().wires.len(), 1);
}

// --- Merge after placing ---------------------------------------------------

#[test]
fn a_wire_drawn_straight_on_from_another_folds_into_it() {
    let (mut engine, first) = engine_with_first_leg();
    place(&mut engine, new_wire((45.0, 50.0), (55.0, 50.0)));

    assert_eq!(spans(&engine), [((35.0, 50.0), (55.0, 50.0))]);
    assert_eq!(
        engine.document().wires[0].uuid,
        first,
        "the older id survives"
    );
}

#[test]
fn a_fold_never_changes_the_nets() {
    let (mut engine, _) = engine_with_first_leg();
    let mut unmerged = engine.document().clone();
    unmerged.wires.push(new_wire((45.0, 50.0), (55.0, 50.0)));
    place(&mut engine, unmerged.wires[1].clone());

    assert_eq!(engine.document().wires.len(), 1, "it merged");
    assert_eq!(unmerged.wires.len(), 2);
    assert_eq!(net_shape(engine.document()), net_shape(&unmerged));
    let nets = net_shape(engine.document());
    assert_eq!(nets.len(), 1, "R1.2 and R2.1 are one net");
    assert_eq!(nets[0].1.len(), 2);
}

#[test]
fn the_fold_is_part_of_the_same_undo_step() {
    let (mut engine, _) = engine_with_first_leg();
    let before = spans(&engine);
    place(&mut engine, new_wire((45.0, 50.0), (55.0, 50.0)));
    assert_eq!(engine.document().wires.len(), 1);

    assert!(engine.undo().is_ok_and(|undone| undone.is_some()));
    assert_eq!(spans(&engine), before);
}

#[test]
fn a_wire_that_closes_a_gap_folds_all_three_into_one() {
    let mut sheet = SchematicSheet::default();
    add_wire(&mut sheet, (30.0, 50.0), (40.0, 50.0));
    add_wire(&mut sheet, (50.0, 50.0), (60.0, 50.0));
    let mut engine = Engine::new(sheet).expect("engine");
    place(&mut engine, new_wire((40.0, 50.0), (50.0, 50.0)));
    assert_eq!(spans(&engine), [((30.0, 50.0), (60.0, 50.0))]);
}

/// Each case: a first leg (35,50)-(45,50), `extra` placed on the shared
/// point (45,50), then a wire from (45,50) that would otherwise fold in.
fn assert_no_fold(name: &str, build: impl FnOnce(&mut SchematicSheet), next: Wire) {
    let mut sheet = two_resistors();
    add_wire(&mut sheet, (35.0, 50.0), (45.0, 50.0));
    build(&mut sheet);
    let wires_before = sheet.wires.len();
    let mut engine = Engine::new(sheet).expect("engine");
    place(&mut engine, next);
    assert_eq!(engine.document().wires.len(), wires_before + 1, "{name}");
}

#[test]
fn nothing_folds_where_something_else_attaches() {
    let straight = || new_wire((45.0, 50.0), (55.0, 50.0));
    assert_no_fold(
        "a T: a third wire ends there",
        |s| {
            add_wire(s, (45.0, 50.0), (45.0, 60.0));
        },
        straight(),
    );
    assert_no_fold(
        "a wire passes through",
        |s| {
            add_wire(s, (45.0, 40.0), (45.0, 60.0));
        },
        straight(),
    );
    assert_no_fold(
        "a junction",
        |s| {
            s.junctions.push(Junction {
                uuid: Uuid::new_v4(),
                position: pt(45.0, 50.0),
                ..Default::default()
            });
        },
        straight(),
    );
    assert_no_fold(
        "a label",
        |s| {
            s.labels.push(Label {
                uuid: Uuid::new_v4(),
                text: "SIG".into(),
                position: pt(45.0, 50.0),
                rotation: 0.0,
                label_type: LabelType::Net,
                shape: String::new(),
                font_size: 1.27,
                font_family: None,
                justify: Default::default(),
                justify_v: signex_types::schematic::VAlign::Bottom,
                locked: false,
                unknown_columns: Default::default(),
            });
        },
        straight(),
    );
    assert_no_fold(
        "a pin",
        |s| {
            // R3's pin 1 sits at (45, 50).
            add_resistor(s, "R3", pt(50.0, 50.0));
        },
        straight(),
    );
}

#[test]
fn nothing_folds_that_is_not_a_straight_continuation() {
    assert_no_fold("a bend", |_| {}, new_wire((45.0, 50.0), (45.0, 60.0)));
    assert_no_fold(
        "doubling back",
        |_| {},
        new_wire((45.0, 50.0), (40.0, 50.0)),
    );
    assert_no_fold(
        "overlapping but not touching",
        |_| {},
        new_wire((40.0, 50.0), (55.0, 50.0)),
    );
    assert_no_fold(
        "a different width",
        |_| {},
        Wire {
            stroke_width: 0.4,
            ..new_wire((45.0, 50.0), (55.0, 50.0))
        },
    );
}

#[test]
fn diagonal_wires_never_fold() {
    let mut sheet = SchematicSheet::default();
    add_wire(&mut sheet, (0.0, 0.0), (10.0, 10.0));
    let mut engine = Engine::new(sheet).expect("engine");
    place(&mut engine, new_wire((10.0, 10.0), (20.0, 20.0)));
    assert_eq!(engine.document().wires.len(), 2);
}

#[test]
fn a_locked_wire_is_never_folded_away() {
    let (mut engine, first) = engine_with_first_leg();
    engine
        .execute(Command::SetItemLocked {
            item: wire_item(first),
            locked: true,
        })
        .unwrap();
    place(&mut engine, new_wire((45.0, 50.0), (55.0, 50.0)));
    assert_eq!(engine.document().wires.len(), 2);
}

#[test]
fn a_pair_that_already_met_end_to_end_is_not_folded_by_an_unrelated_edit() {
    let mut sheet = SchematicSheet::default();
    add_wire(&mut sheet, (0.0, 0.0), (10.0, 0.0));
    add_wire(&mut sheet, (10.0, 0.0), (20.0, 0.0));
    let mut engine = Engine::new(sheet).expect("engine");
    place(&mut engine, new_wire((0.0, 30.0), (10.0, 30.0)));
    assert_eq!(engine.document().wires.len(), 3);
}

// --- Merge after moving ----------------------------------------------------

#[test]
fn moving_a_wire_into_line_folds_it_in_one_undo_step() {
    let mut sheet = SchematicSheet::default();
    let first = add_wire(&mut sheet, (35.0, 50.0), (45.0, 50.0));
    let other = add_wire(&mut sheet, (45.0, 60.0), (55.0, 60.0));
    let mut engine = Engine::new(sheet).expect("engine");
    let before = spans(&engine);

    engine
        .execute(Command::MoveSelection {
            items: vec![wire_item(other)],
            dx: 0.0,
            dy: -10.0,
            keep_wires: false,
        })
        .unwrap();
    assert_eq!(spans(&engine), [((35.0, 50.0), (55.0, 50.0))]);
    assert_eq!(engine.document().wires[0].uuid, first);

    assert!(engine.undo().is_ok_and(|undone| undone.is_some()));
    assert_eq!(spans(&engine), before);
}

#[test]
fn moving_both_halves_of_a_broken_wire_does_not_fold_them_again() {
    let (mut engine, wire) = engine_with_one_wire();
    engine
        .execute(Command::BreakWire {
            wire,
            at: pt(8.0, 10.0),
        })
        .unwrap();
    let halves: Vec<SelectedItem> = engine
        .document()
        .wires
        .iter()
        .map(|w| wire_item(w.uuid))
        .collect();

    engine
        .execute(Command::MoveSelection {
            items: halves,
            dx: 3.0,
            dy: 5.0,
            keep_wires: false,
        })
        .unwrap();
    assert_eq!(engine.document().wires.len(), 2);
}

#[test]
fn a_move_that_folds_never_changes_the_nets() {
    let mut sheet = two_resistors();
    add_wire(&mut sheet, (35.0, 50.0), (45.0, 50.0));
    let other = add_wire(&mut sheet, (45.0, 60.0), (55.0, 60.0));
    add_wire(&mut sheet, (55.0, 60.0), (55.0, 50.0));
    let mut engine = Engine::new(sheet).expect("engine");

    // The same wires, moved without a merge pass, are the reference.
    let mut reference = engine.document().clone();
    reference.wires[1].start.y = 50.0;
    reference.wires[1].end.y = 50.0;
    reference.wires[2].start.y = 60.0;

    engine
        .execute(Command::MoveSelection {
            items: vec![wire_item(other)],
            dx: 0.0,
            dy: -10.0,
            keep_wires: false,
        })
        .unwrap();
    assert!(engine.document().wires.len() < 3, "it folded");
    assert_eq!(net_shape(engine.document()), net_shape(&reference));
}

/// A wire end resting on another wire's side without a junction is no
/// connection (#107). Folding must not "repair" it by minting a dot there.
#[test]
fn a_fold_leaves_a_junctionless_t_at_the_far_end_alone() {
    let mut sheet = two_resistors();
    add_wire(&mut sheet, (35.0, 40.0), (35.0, 60.0));
    add_wire(&mut sheet, (35.0, 50.0), (45.0, 50.0));
    let mut unmerged = sheet.clone();
    unmerged.wires.push(new_wire((45.0, 50.0), (55.0, 50.0)));
    let mut engine = Engine::new(sheet).expect("engine");

    place(&mut engine, unmerged.wires[2].clone());

    assert_eq!(engine.document().wires.len(), 2, "it folded");
    assert!(engine.document().junctions.is_empty(), "no dot was minted");
    assert_eq!(net_shape(engine.document()), net_shape(&unmerged));
}

fn with_foreign_style(mut wire: Wire, value: &str) -> Wire {
    wire.unknown_columns =
        serde_json::from_value(serde_json::json!([["future_style", value]])).unwrap();
    wire
}

/// Foreign columns belong to one wire; folding would silently drop one set.
#[test]
fn wires_with_different_foreign_columns_are_not_folded() {
    let mut sheet = two_resistors();
    add_wire(&mut sheet, (35.0, 50.0), (45.0, 50.0));
    sheet.wires[0] = with_foreign_style(sheet.wires[0].clone(), "solid");
    let mut engine = Engine::new(sheet).expect("engine");
    let dashed = with_foreign_style(new_wire((45.0, 50.0), (55.0, 50.0)), "dashed");
    place(&mut engine, dashed.clone());
    assert_eq!(engine.document().wires.len(), 2);
    assert_eq!(
        engine.document().wires[1].unknown_columns,
        dashed.unknown_columns
    );
}

#[test]
fn wires_with_equal_foreign_columns_still_fold() {
    let mut sheet = two_resistors();
    add_wire(&mut sheet, (35.0, 50.0), (45.0, 50.0));
    sheet.wires[0] = with_foreign_style(sheet.wires[0].clone(), "dashed");
    let mut engine = Engine::new(sheet).expect("engine");
    place(
        &mut engine,
        with_foreign_style(new_wire((45.0, 50.0), (55.0, 50.0)), "dashed"),
    );
    assert_eq!(engine.document().wires.len(), 1);
}

/// Two wires ending on a third wire's side with no junction are not connected
/// to it (#107). Folding them would still be safe, but the merge refuses any
/// point another wire runs through.
#[test]
fn a_wire_running_through_the_shared_point_blocks_the_fold() {
    let mut sheet = two_resistors();
    add_wire(&mut sheet, (35.0, 50.0), (45.0, 50.0));
    add_wire(&mut sheet, (45.0, 50.0), (55.0, 50.0));
    add_wire(&mut sheet, (45.0, 40.0), (45.0, 60.0));
    let before = net_shape(&sheet);
    let mut engine = Engine::new(sheet).expect("engine");

    assert!(!engine.merge_collinear_wires(&crate::TouchingPairs::new()));
    assert_eq!(engine.document().wires.len(), 3);
    assert_eq!(net_shape(engine.document()), before);
}

fn red() -> signex_types::theme::Color {
    signex_types::theme::Color::new(255, 0, 0, 255)
}

#[test]
fn wires_of_different_net_colours_are_not_folded_and_equal_ones_are() {
    for (first, second, folds) in [
        (Some(red()), None, false),
        (None, Some(red()), false),
        (
            Some(red()),
            Some(signex_types::theme::Color::new(0, 255, 0, 255)),
            false,
        ),
        (Some(red()), Some(red()), true),
        (None, None, true),
    ] {
        let (mut engine, a) = engine_with_first_leg();
        let b = new_wire((45.0, 50.0), (55.0, 50.0));
        let mut colors = std::collections::HashMap::new();
        colors.extend(first.map(|c| (a, c)));
        colors.extend(second.map(|c| (b.uuid, c)));
        engine.set_wire_colors(colors);
        place(&mut engine, b);
        assert_eq!(
            engine.document().wires.len() == 1,
            folds,
            "{first:?} {second:?}"
        );
    }
}

#[test]
fn break_wire_is_refused_where_a_pin_tip_rests_on_the_wire() {
    let mut sheet = SchematicSheet::default();
    let wire = add_wire(&mut sheet, (0.0, 10.0), (20.0, 10.0));
    // R1's pin 1 tip is at (8, 10), on the wire's side and not connected.
    add_resistor(&mut sheet, "R1", pt(13.0, 10.0));
    add_resistor(&mut sheet, "R2", pt(25.0, 10.0));
    let before = net_shape(&sheet);
    let mut engine = Engine::new(sheet).expect("engine");

    let refused = engine.execute(Command::BreakWire {
        wire,
        at: pt(8.0, 10.0),
    });

    assert!(matches!(refused, Err(EngineError::BreakWireBlocked)));
    assert_eq!(engine.document().wires.len(), 1);
    assert_eq!(net_shape(engine.document()), before);
}

#[test]
fn break_wire_is_refused_where_another_wire_ends_on_the_side() {
    let mut sheet = SchematicSheet::default();
    let wire = add_wire(&mut sheet, (0.0, 10.0), (20.0, 10.0));
    add_wire(&mut sheet, (8.0, 10.0), (8.0, 30.0));
    let before = net_shape(&sheet);
    let mut engine = Engine::new(sheet).expect("engine");

    let refused = engine.execute(Command::BreakWire {
        wire,
        at: pt(8.0, 10.0),
    });

    assert!(matches!(refused, Err(EngineError::BreakWireBlocked)));
    assert_eq!(engine.document().wires.len(), 2);
    assert_eq!(net_shape(engine.document()), before);
}

#[test]
fn break_wire_is_refused_where_another_wire_crosses_without_a_dot() {
    let mut sheet = SchematicSheet::default();
    let wire = add_wire(&mut sheet, (0.0, 10.0), (20.0, 10.0));
    add_wire(&mut sheet, (8.0, 0.0), (8.0, 30.0));
    let before = net_shape(&sheet);
    let mut engine = Engine::new(sheet).expect("engine");

    let refused = engine.execute(Command::BreakWire {
        wire,
        at: pt(8.0, 10.0),
    });

    assert!(matches!(refused, Err(EngineError::BreakWireBlocked)));
    assert_eq!(engine.document().wires.len(), 2);
    assert!(engine.document().junctions.is_empty());
    assert_eq!(net_shape(engine.document()), before);
}
