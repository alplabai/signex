//! A pin end inside a wire joins it through a dot (2026-11-18): the edit that
//! makes the contact mints the dot in its own undo step, a dragged wire then
//! carries the pin with a stub, and a sheet that already had the bare contact
//! is reported and repaired in one step, never changed on open.

use crate::rubber_band_tests::{
    add_resistor, add_wire, has, lib_pin, lib_symbol, mv, net_shape, place, segments,
};
use crate::test_support::test_sheet;
use crate::{Command, Engine, bare_pin_taps, pin_junction_repairs};
use signex_types::object_id::SequentialIds;
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind};
use uuid::Uuid;

/// A one-pin part whose pin tip is at `at`.
fn add_pin(sheet: &mut SchematicSheet, reference: &str, at: (f64, f64)) -> Uuid {
    sheet
        .lib_symbols
        .entry("TP".to_string())
        .or_insert_with(|| lib_symbol("TP", vec![lib_pin("1", 0.0)]));
    place(sheet, reference, "TP", Point::new(at.0, at.1))
}

/// Place another one-pin part through the engine, its pin tip at `at`.
fn place_part(engine: &mut Engine, reference: &str, at: (f64, f64)) -> Uuid {
    let mut part = engine.document().symbols[0].clone();
    part.uuid = Uuid::now_v7();
    part.reference = reference.to_string();
    part.position = Point::new(at.0, at.1);
    let id = part.uuid;
    engine
        .execute(Command::PlaceSymbol { symbol: part })
        .unwrap();
    id
}

fn dots(engine: &Engine) -> Vec<Point> {
    let sheet = engine.document();
    sheet.junctions.iter().map(|j| j.position).collect()
}

/// Whether one net holds every part of `ids` (pin 1 each).
fn one_net(sheet: &SchematicSheet, ids: &[Uuid]) -> bool {
    net_shape(sheet)
        .into_iter()
        .any(|(_, pins)| ids.iter().all(|id| pins.contains(&(*id, "1".to_string()))))
}

/// J1's pin at the left end of a wire (20,40)-(80,40).
fn wire_from_j1() -> (Engine, Uuid, Uuid) {
    let mut sheet = test_sheet();
    let j1 = add_pin(&mut sheet, "J1", (20.0, 40.0));
    let wire = add_wire(&mut sheet, (20.0, 40.0), (80.0, 40.0));
    (Engine::new(sheet).unwrap(), j1, wire)
}

#[test]
fn placing_a_part_with_a_pin_end_inside_a_wire_joins_it_through_a_dot() {
    let (mut engine, j1, _) = wire_from_j1();
    let tp1 = place_part(&mut engine, "TP1", (50.0, 40.0));
    assert_eq!(dots(&engine), vec![Point::new(50.0, 40.0)]);
    let dot = &engine.document().junctions[0];
    assert!(dot.minted, "an automatic dot");
    assert!(one_net(engine.document(), &[j1, tp1]));

    // The dot is part of the placement's undo step.
    engine.undo().unwrap();
    assert!(dots(&engine).is_empty());
    assert_eq!(engine.document().symbols.len(), 1);
}

#[test]
fn a_part_on_a_wire_end_needs_no_dot() {
    let (mut engine, _, _) = wire_from_j1();
    place_part(&mut engine, "TP1", (80.0, 40.0));
    assert!(dots(&engine).is_empty());
}

#[test]
fn drawing_a_wire_through_a_pin_end_joins_it_and_moving_the_part_off_drops_the_dot() {
    let mut sheet = test_sheet();
    let tp1 = add_pin(&mut sheet, "TP1", (50.0, 40.0));
    let mut engine = Engine::new(sheet).unwrap();
    let wire = signex_types::schematic::Wire {
        uuid: Uuid::now_v7(),
        start: Point::new(20.0, 40.0),
        end: Point::new(80.0, 40.0),
        ..Default::default()
    };
    engine
        .execute(Command::PlaceWireSegmentAsIs { wire })
        .unwrap();
    assert_eq!(dots(&engine), vec![Point::new(50.0, 40.0)]);

    let part = SelectedItem::new(tp1, SelectedKind::Symbol);
    mv(&mut engine, vec![part], 0.0, -20.0, false);
    assert!(dots(&engine).is_empty(), "the pin left: its dot goes");
}

/// The FB1 / F1 case: parts placed on a wire mid-span get their dots, so a
/// drag of the wire leaves a stub to each, and every pin stays on the net.
#[test]
fn a_dragged_wire_keeps_the_parts_placed_on_it() {
    let (mut engine, j1, segment) = wire_from_j1();
    let mut parts = vec![j1];
    for (name, x) in [("FB1", 35.0), ("F1", 50.0), ("L", 65.0)] {
        parts.push(place_part(&mut engine, name, (x, 40.0)));
    }
    let before = net_shape(engine.document());
    assert!(one_net(engine.document(), &parts));

    let item = SelectedItem::new(segment, SelectedKind::Wire);
    mv(&mut engine, vec![item], 0.0, 10.0, true);
    let segs = segments(&engine);
    for x in [20.0, 35.0, 50.0, 65.0] {
        assert!(has(&segs, (x, 40.0), (x, 50.0)), "{x}: {segs:?}");
    }
    assert_eq!(net_shape(engine.document()), before, "every pin stays");
}

#[test]
fn an_old_bare_contact_is_reported_untouched_and_repaired_in_one_step() {
    let mut sheet = test_sheet();
    let j1 = add_pin(&mut sheet, "J1", (20.0, 40.0));
    let tp1 = add_pin(&mut sheet, "TP1", (50.0, 40.0));
    add_wire(&mut sheet, (20.0, 40.0), (80.0, 40.0));

    // Opening reads the sheet as it is: the contact is reported, not fixed.
    let mut engine = Engine::new(sheet).unwrap();
    let taps = bare_pin_taps(engine.document());
    assert_eq!(taps, vec![Point::new(50.0, 40.0)]);
    assert!(dots(&engine).is_empty());
    assert!(!one_net(engine.document(), &[j1, tp1]));

    let repairs = pin_junction_repairs(engine.document(), &mut SequentialIds::default());
    engine.execute_batch(repairs).unwrap();
    assert_eq!(dots(&engine), vec![Point::new(50.0, 40.0)]);
    assert!(one_net(engine.document(), &[j1, tp1]));
    assert!(bare_pin_taps(engine.document()).is_empty());

    engine.undo().unwrap();
    assert!(dots(&engine).is_empty(), "one undo step");
}

/// A wire drawn along a part through both its pin ends would short the part
/// if each end got a dot: neither does, and the two pins stay apart.
#[test]
fn a_wire_through_both_pins_of_one_part_does_not_short_it() {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 40.0)); // pins 45, 55
    let mut engine = Engine::new(sheet).unwrap();
    let wire = signex_types::schematic::Wire {
        uuid: Uuid::now_v7(),
        start: Point::new(20.0, 40.0),
        end: Point::new(80.0, 40.0),
        ..Default::default()
    };
    engine
        .execute(Command::PlaceWireSegmentAsIs { wire })
        .unwrap();
    assert!(dots(&engine).is_empty());
    let shorted = net_shape(engine.document()).into_iter().any(|(_, pins)| {
        pins.contains(&(r1, "1".to_string())) && pins.contains(&(r1, "2".to_string()))
    });
    assert!(!shorted, "R1's pins stay apart");
}

/// A bare touch an older sheet holds stays bare when an edit reshapes its wire
/// (here the wire grows by its end grip): the edit did not make the contact.
#[test]
fn reshaping_a_wire_leaves_an_old_bare_touch_bare() {
    let mut sheet = test_sheet();
    add_pin(&mut sheet, "J1", (20.0, 40.0));
    add_pin(&mut sheet, "TP1", (50.0, 40.0));
    let wire = add_wire(&mut sheet, (20.0, 40.0), (80.0, 40.0));
    let mut engine = Engine::new(sheet).unwrap();
    engine
        .execute(Command::MoveWireEnd {
            wire,
            start: false,
            to: Point::new(90.0, 40.0),
            keep_wires: true,
        })
        .unwrap();
    assert!(dots(&engine).is_empty());
    assert_eq!(bare_pin_taps(engine.document()).len(), 1, "still reported");
}

/// An automatic pin dot goes only when its pin or its wire leaves (decision
/// 2026-11-18): R1 stands across wires A and B, each pin dotted; a wire then
/// joining A and B shorts R1 by the user's own hand, and both dots stay.
#[test]
fn an_unrelated_edit_keeps_a_pin_dot_whose_pin_and_wire_stay() {
    let mut sheet = test_sheet();
    let r1 = add_resistor(&mut sheet, "R1", Point::new(50.0, 45.0));
    let mut part = sheet.symbols.pop().expect("R1");
    part.rotation = 90.0; // pins at (50, 40) and (50, 50)
    add_wire(&mut sheet, (30.0, 40.0), (70.0, 40.0)); // A
    add_wire(&mut sheet, (30.0, 50.0), (70.0, 50.0)); // B
    let mut engine = Engine::new(sheet).unwrap();
    engine
        .execute(Command::PlaceSymbol { symbol: part })
        .unwrap();
    assert_eq!(dots(&engine).len(), 2, "each pin end on its wire is dotted");

    let joint = signex_types::schematic::Wire {
        uuid: Uuid::now_v7(),
        start: Point::new(30.0, 40.0),
        end: Point::new(30.0, 50.0),
        ..Default::default()
    };
    engine
        .execute(Command::PlaceWireSegmentAsIs { wire: joint })
        .unwrap();
    let mut left = dots(&engine);
    left.sort_by(|a, b| a.y.total_cmp(&b.y));
    assert_eq!(left, vec![Point::new(50.0, 40.0), Point::new(50.0, 50.0)]);
    let both = net_shape(engine.document()).into_iter().any(|(_, pins)| {
        pins.contains(&(r1, "1".to_string())) && pins.contains(&(r1, "2".to_string()))
    });
    assert!(both, "R1's pins stay on their wires, now one net");
}

/// J1's wire A (0,40)-(60,40) and J2's wire B (40,40)-(100,40) overlap from
/// x 40 to 60 on two nets: an end inside the other wire is no connection.
fn two_net_overlap(sheet: &mut SchematicSheet) -> (Uuid, Uuid) {
    let j1 = add_pin(sheet, "J1", (0.0, 40.0));
    let j2 = add_pin(sheet, "J2", (100.0, 40.0));
    add_wire(sheet, (0.0, 40.0), (60.0, 40.0));
    add_wire(sheet, (40.0, 40.0), (100.0, 40.0));
    (j1, j2)
}

/// The notices of the last commands that name a pin end left bare on wires
/// of two nets.
fn crossed_notices(engine: &mut Engine) -> Vec<String> {
    (engine.take_warnings().into_iter())
        .filter(|w| w.starts_with("could not join"))
        .collect()
}

/// A pin dot never merges two nets (maintainer decision 2026-11-19): a part
/// moved so its pin end lands where wires of two nets overlap gets no dot,
/// both nets stay apart, and one notice names the pin and both nets.
#[test]
fn a_pin_moved_onto_wires_of_two_nets_gets_no_dot_and_a_notice() {
    let mut sheet = test_sheet();
    let (j1, j2) = two_net_overlap(&mut sheet);
    let tp1 = add_pin(&mut sheet, "TP1", (50.0, 10.0));
    let mut engine = Engine::new(sheet).unwrap();
    assert!(!one_net(engine.document(), &[j1, j2]), "two nets to start");

    let part = SelectedItem::new(tp1, SelectedKind::Symbol);
    mv(&mut engine, vec![part], 0.0, 30.0, false);
    assert!(dots(&engine).is_empty(), "no dot: {:?}", dots(&engine));
    let sheet = engine.document();
    assert!(!one_net(sheet, &[j1, j2]), "J1 and J2 stay apart");
    assert!(!one_net(sheet, &[tp1, j1]) && !one_net(sheet, &[tp1, j2]));
    let notices = crossed_notices(&mut engine);
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert!(
        notices[0].contains("TP1 pin 1") && notices[0].contains("two nets"),
        "{notices:?}"
    );
}

/// The same landing with "keep wires" on: TP1 keeps its wire to J3, its pin
/// end still lands inside wires of two nets, and still no dot merges them.
#[test]
fn a_kept_pin_moved_onto_wires_of_two_nets_gets_no_dot_and_a_notice() {
    let mut sheet = test_sheet();
    let (j1, j2) = two_net_overlap(&mut sheet);
    let tp1 = add_pin(&mut sheet, "TP1", (50.0, 10.0));
    add_pin(&mut sheet, "J3", (20.0, 10.0));
    add_wire(&mut sheet, (20.0, 10.0), (50.0, 10.0));
    let mut engine = Engine::new(sheet).unwrap();

    let part = SelectedItem::new(tp1, SelectedKind::Symbol);
    mv(&mut engine, vec![part], 0.0, 30.0, true);
    let tip = Point::new(50.0, 40.0);
    assert!(!dots(&engine).contains(&tip), "no dot: {:?}", dots(&engine));
    let sheet = engine.document();
    assert!(!one_net(sheet, &[j1, j2]), "J1 and J2 stay apart");
    assert!(!one_net(sheet, &[tp1, j1]) && !one_net(sheet, &[tp1, j2]));
    // Every notice, in order: no path to the pin end can avoid the two-net
    // place, so the guard leaves TP1 as a plain move and says so, then the
    // commit names the dot it refused.
    let warnings = engine.take_warnings();
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert_eq!(
        warnings[0], "could not keep TP1 pin 1 connected without crossing another net",
        "{warnings:?}"
    );
    assert!(
        warnings[1]
            .starts_with("could not join TP1 pin 1 to the wires it ends on: they carry two nets, "),
        "{warnings:?}"
    );
}

/// The reviewer's bare T (2026-11-19): J3's wire ends inside J1-J2's wire
/// with no dot, so the two are separate nets. TP1 moved so its pin end lands
/// on that point gets no dot, which would merge them, and one notice.
#[test]
fn a_pin_moved_onto_a_bare_t_of_two_nets_gets_no_dot_and_a_notice() {
    let mut sheet = test_sheet();
    let j1 = add_pin(&mut sheet, "J1", (0.0, 40.0));
    let j2 = add_pin(&mut sheet, "J2", (100.0, 40.0));
    add_wire(&mut sheet, (0.0, 40.0), (100.0, 40.0));
    let j3 = add_pin(&mut sheet, "J3", (50.0, 80.0));
    add_wire(&mut sheet, (50.0, 80.0), (50.0, 40.0));
    let tp1 = add_pin(&mut sheet, "TP1", (50.0, 10.0));
    let mut engine = Engine::new(sheet).unwrap();
    assert!(one_net(engine.document(), &[j1, j2]));
    assert!(
        !one_net(engine.document(), &[j1, j3]),
        "a bare T joins nothing"
    );

    let part = SelectedItem::new(tp1, SelectedKind::Symbol);
    mv(&mut engine, vec![part], 0.0, 30.0, false);
    assert!(dots(&engine).is_empty(), "no dot: {:?}", dots(&engine));
    let sheet = engine.document();
    assert!(!one_net(sheet, &[j1, j3]), "the T stays bare");
    assert!(!one_net(sheet, &[tp1, j1]), "TP1 does not join J1-J2");
    let warnings = engine.take_warnings();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(
        warnings[0]
            .starts_with("could not join TP1 pin 1 to the wires it ends on: they carry two nets, "),
        "{warnings:?}"
    );
}

/// Two wires of different nets crossing at right angles, with no dot: a pin
/// end moved onto the crossing gets no dot and one notice.
#[test]
fn a_pin_moved_onto_a_crossing_of_two_nets_gets_no_dot_and_a_notice() {
    let mut sheet = test_sheet();
    let j1 = add_pin(&mut sheet, "J1", (0.0, 40.0));
    add_wire(&mut sheet, (0.0, 40.0), (100.0, 40.0));
    let j3 = add_pin(&mut sheet, "J3", (50.0, 80.0));
    add_wire(&mut sheet, (50.0, 80.0), (50.0, 0.0));
    let tp1 = add_pin(&mut sheet, "TP1", (70.0, 10.0));
    let mut engine = Engine::new(sheet).unwrap();

    let part = SelectedItem::new(tp1, SelectedKind::Symbol);
    mv(&mut engine, vec![part], -20.0, 30.0, false);
    assert!(dots(&engine).is_empty(), "no dot: {:?}", dots(&engine));
    let sheet = engine.document();
    assert!(!one_net(sheet, &[j1, j3]), "the crossing stays a crossing");
    assert!(!one_net(sheet, &[tp1, j1]) && !one_net(sheet, &[tp1, j3]));
    assert_eq!(crossed_notices(&mut engine).len(), 1);
}

/// The wire side: two wires of different nets moved so they cross on a pin
/// end that touched nothing before. No dot joins the three, one notice.
#[test]
fn wires_moved_to_cross_on_a_pin_end_get_no_dot_and_a_notice() {
    let mut sheet = test_sheet();
    let j1 = add_pin(&mut sheet, "J1", (0.0, 30.0));
    let row = add_wire(&mut sheet, (0.0, 30.0), (100.0, 30.0));
    let j3 = add_pin(&mut sheet, "J3", (60.0, 80.0));
    let column = add_wire(&mut sheet, (60.0, 80.0), (60.0, 0.0));
    let tp1 = add_pin(&mut sheet, "TP1", (50.0, 40.0));
    let mut engine = Engine::new(sheet).unwrap();
    assert!(
        bare_pin_taps(engine.document()).is_empty(),
        "TP1 touches nothing"
    );

    // The row down onto TP1's row and the column left onto TP1's column, each
    // with its part, so J1 and J3 keep their wires and their two nets.
    engine
        .execute(Command::MoveEach {
            moves: vec![
                (SelectedItem::new(row, SelectedKind::Wire), 0.0, 10.0),
                (SelectedItem::new(j1, SelectedKind::Symbol), 0.0, 10.0),
                (SelectedItem::new(column, SelectedKind::Wire), -10.0, 0.0),
                (SelectedItem::new(j3, SelectedKind::Symbol), -10.0, 0.0),
            ],
            keep_wires: false,
            wiring: signex_types::corner::DragWiringMode::Auto,
        })
        .unwrap();
    assert!(
        !dots(&engine).contains(&Point::new(50.0, 40.0)),
        "no dot: {:?}",
        dots(&engine)
    );
    let sheet = engine.document();
    assert!(!one_net(sheet, &[j1, j3]), "the crossing joins nothing");
    assert!(!one_net(sheet, &[tp1, j1]) && !one_net(sheet, &[tp1, j3]));
    let notices = crossed_notices(&mut engine);
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert!(notices[0].contains("TP1 pin 1"), "{notices:?}");
}

/// A drag preview's throwaway engine names nothing: the notice belongs to
/// the commit, so the ghost pushes none frame by frame.
#[test]
fn a_drag_preview_pushes_no_notice_for_a_refused_dot() {
    let mut sheet = test_sheet();
    two_net_overlap(&mut sheet);
    let tp1 = add_pin(&mut sheet, "TP1", (50.0, 10.0));
    let preview = crate::MovePreview::new(&sheet);
    let part = SelectedItem::new(tp1, SelectedKind::Symbol);
    for keep_wires in [false, true] {
        let (mut engine, _, result) = preview
            .moved_report(&[(part, 0.0, 30.0)], keep_wires)
            .expect("a preview engine");
        assert!(matches!(result, Some(Ok(_))), "the move ran");
        let tip = Point::new(50.0, 40.0);
        assert!(
            !dots(&engine).contains(&tip),
            "the ghost shows no dot either"
        );
        let warnings = engine.take_warnings();
        assert!(
            !warnings.iter().any(|w| w.starts_with("could not join")),
            "{warnings:?}"
        );
    }
}

/// A pin end already sitting where wires of two nets overlap is no bare touch
/// to repair: a dot there would merge the nets.
#[test]
fn a_touch_on_wires_of_two_nets_is_not_reported_for_repair() {
    let mut sheet = test_sheet();
    two_net_overlap(&mut sheet);
    add_pin(&mut sheet, "TP1", (50.0, 40.0));
    let engine = Engine::new(sheet).unwrap();
    assert!(bare_pin_taps(engine.document()).is_empty());
    let repairs = pin_junction_repairs(engine.document(), &mut SequentialIds::default());
    assert!(repairs.is_empty());
}
