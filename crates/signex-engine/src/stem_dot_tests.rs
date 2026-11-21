//! A directive standing on a wire by a stem gets a junction dot at its foot
//! (Caner 2026-11-21): a Net Parameters flag, a Differential Pair with a
//! class. The dot follows the directive in its own undo step; a net label is
//! not a stem and gets none.

use crate::rubber_band_tests::{add_wire, mv};
use crate::test_support::test_sheet;
use crate::{Command, Engine};
use signex_types::schematic::{
    Label, LabelType, NetRules, Point, SchDirective, SchematicSheet, SelectedItem, SelectedKind,
};
use uuid::Uuid;

fn dots(engine: &Engine) -> Vec<Point> {
    (engine.document().junctions.iter())
        .map(|j| j.position)
        .collect()
}

fn net_parameters(at: (f64, f64)) -> SchDirective {
    SchDirective::NetParameters {
        uuid: Uuid::now_v7(),
        position: Point::new(at.0, at.1),
        net_class: None,
        diff_pair_class: None,
        rules: NetRules::default(),
        show_on_flag: false,
    }
}

fn diff_pair(at: (f64, f64), class: Option<&str>) -> SchDirective {
    SchDirective::DiffPair {
        uuid: Uuid::now_v7(),
        position: Point::new(at.0, at.1),
        class: class.map(str::to_string),
    }
}

/// One wire (0,40)-(100,40).
fn one_wire() -> Engine {
    let mut sheet = test_sheet();
    add_wire(&mut sheet, (0.0, 40.0), (100.0, 40.0));
    Engine::new(sheet).unwrap()
}

fn place(engine: &mut Engine, directive: SchDirective) -> SelectedItem {
    let item = SelectedItem::new(directive.uuid(), SelectedKind::Directive);
    engine
        .execute(Command::PlaceDirective { directive })
        .unwrap();
    item
}

#[test]
fn each_stem_directive_inside_a_wire_gets_a_dot_its_undo_takes() {
    for directive in [
        net_parameters((50.0, 40.0)),
        diff_pair((50.0, 40.0), Some("USB")),
    ] {
        let mut engine = one_wire();
        place(&mut engine, directive);
        assert_eq!(dots(&engine), vec![Point::new(50.0, 40.0)]);
        assert!(engine.document().junctions[0].minted);
        engine.undo().unwrap();
        assert!(dots(&engine).is_empty());
        assert!(engine.document().directives.is_empty());
    }
}

#[test]
fn a_stem_off_every_wire_and_a_pair_with_no_flag_get_no_dot() {
    let mut engine = one_wire();
    place(&mut engine, net_parameters((50.0, 10.0)));
    place(&mut engine, diff_pair((30.0, 40.0), None));
    assert!(dots(&engine).is_empty(), "{:?}", dots(&engine));
}

#[test]
fn moving_the_directive_off_the_wire_drops_the_dot_and_undo_brings_it_back() {
    let mut engine = one_wire();
    let item = place(&mut engine, net_parameters((50.0, 40.0)));
    mv(&mut engine, vec![item], 0.0, -20.0, false);
    assert!(dots(&engine).is_empty(), "{:?}", dots(&engine));
    engine.undo().unwrap();
    assert_eq!(dots(&engine), vec![Point::new(50.0, 40.0)]);

    engine
        .execute(Command::DeleteSelection { items: vec![item] })
        .unwrap();
    assert!(dots(&engine).is_empty(), "the dot leaves with the directive");
}

#[test]
fn a_stem_where_two_wires_cross_gets_no_dot() {
    let mut sheet: SchematicSheet = test_sheet();
    add_wire(&mut sheet, (0.0, 40.0), (100.0, 40.0));
    add_wire(&mut sheet, (50.0, 0.0), (50.0, 80.0));
    let mut engine = Engine::new(sheet).unwrap();
    place(&mut engine, net_parameters((50.0, 40.0)));
    assert!(dots(&engine).is_empty(), "a dot would join two nets");
}

#[test]
fn a_stem_on_a_corner_of_two_wires_gets_a_dot() {
    let mut sheet = test_sheet();
    add_wire(&mut sheet, (50.0, 0.0), (50.0, 40.0));
    add_wire(&mut sheet, (50.0, 40.0), (100.0, 40.0));
    let mut engine = Engine::new(sheet).unwrap();
    place(&mut engine, net_parameters((50.0, 40.0)));
    assert_eq!(dots(&engine), vec![Point::new(50.0, 40.0)]);
}

#[test]
fn a_net_label_on_a_wire_still_gets_no_dot() {
    let mut engine = one_wire();
    let label = Label::new("SENSE", Point::new(50.0, 40.0), LabelType::Net);
    engine.execute(Command::PlaceLabel { label }).unwrap();
    assert!(dots(&engine).is_empty());
}
