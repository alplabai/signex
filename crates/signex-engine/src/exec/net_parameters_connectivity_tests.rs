//! #2950: a Net Parameters directive gives a net a class and rules; it never
//! joins, splits or renames a net. Placing, editing, moving and removing one
//! through the engine (and undoing each) leaves the netlist's connectivity as
//! it was: the same nets, members and names. Only `Net::class` may differ.

use crate::rubber_band_tests::{lib_pin, lib_symbol, place};
use crate::test_support::test_sheet;
use crate::*;
use signex_types::corner::DragWiringMode;
use signex_types::net::Netlist;
use signex_types::schematic::{
    Junction, Label, LabelType, NetRules, Point, SchDirective, SchematicSheet, SelectedItem,
    SelectedKind, Wire,
};
use uuid::Uuid;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// Two labelled nets: `SW` is two wires meeting at a junction, `VBUS` one
/// wire, 10 mm below.
fn sheet() -> SchematicSheet {
    let mut sheet = test_sheet();
    let wire = |n, a: Point, b: Point| Wire {
        uuid: id(n),
        start: a,
        end: b,
        ..Default::default()
    };
    sheet
        .wires
        .push(wire(1, Point::new(0.0, 0.0), Point::new(10.0, 0.0)));
    sheet
        .wires
        .push(wire(2, Point::new(10.0, 0.0), Point::new(20.0, 0.0)));
    sheet
        .wires
        .push(wire(3, Point::new(0.0, 10.0), Point::new(20.0, 10.0)));
    sheet.junctions.push(Junction {
        uuid: id(4),
        position: Point::new(10.0, 0.0),
        ..Default::default()
    });
    sheet
        .labels
        .push(Label::new("SW", Point::new(0.0, 0.0), LabelType::Net));
    sheet
        .labels
        .push(Label::new("VBUS", Point::new(0.0, 10.0), LabelType::Net));
    // A net needs a pin: a one-pin test point at each wire end.
    sheet
        .lib_symbols
        .entry("TP".to_string())
        .or_insert_with(|| lib_symbol("TP", vec![lib_pin("1", 0.0)]));
    for (n, (x, y)) in [(0.0, 0.0), (20.0, 0.0), (0.0, 10.0), (20.0, 10.0)]
        .into_iter()
        .enumerate()
    {
        place(&mut sheet, &format!("TP{}", n + 1), "TP", Point::new(x, y));
    }
    sheet
}

fn net_parameters(at: Point, class: Option<&str>, rules: NetRules) -> SchDirective {
    SchDirective::NetParameters {
        uuid: id(50),
        position: at,
        net_class: class.map(str::to_string),
        diff_pair_class: None,
        rules,
        show_on_flag: false,
    }
}

/// The netlist with every net's class and dots taken off: what connectivity
/// is. A placed flag's stem gets an automatic dot on its wire (DECISIONS
/// 2026-11-21), which joins no other net; the members and names are kept.
fn connectivity(engine: &Engine) -> Netlist {
    let mut netlist = signex_net::build_netlist(engine.document());
    for net in &mut netlist.nets {
        net.class = None;
        net.junctions.clear();
    }
    netlist
}

fn directive() -> SelectedItem {
    SelectedItem::new(id(50), SelectedKind::Directive)
}

#[test]
fn placing_editing_moving_and_removing_net_parameters_keeps_connectivity() {
    let mut engine = Engine::new(sheet()).unwrap();
    let before = connectivity(&engine);
    assert_eq!(before.nets.len(), 2, "{before:?}");

    engine
        .execute(Command::PlaceDirective {
            directive: net_parameters(Point::new(5.0, 0.0), Some("POWER"), NetRules::default()),
        })
        .unwrap();
    assert_eq!(connectivity(&engine), before, "placed");
    let classed = signex_net::build_netlist(engine.document());
    assert!(
        classed
            .nets
            .iter()
            .any(|n| n.name == "SW" && n.class.is_some()),
        "the directive does reach the netlist: {classed:?}"
    );

    let rules = NetRules {
        width_mm: Some(0.5),
        clearance_mm: Some(0.2),
        ..NetRules::default()
    };
    engine
        .execute(Command::UpdateDirective {
            directive: net_parameters(Point::new(5.0, 0.0), Some("HIGH_CURRENT"), rules),
        })
        .unwrap();
    assert_eq!(connectivity(&engine), before, "edited");

    // Onto the other net.
    engine
        .execute(Command::MoveSelection {
            items: vec![directive()],
            dx: 0.0,
            dy: 10.0,
            keep_wires: false,
            wiring: DragWiringMode::Auto,
        })
        .unwrap();
    assert_eq!(connectivity(&engine), before, "moved");

    engine
        .execute(Command::DeleteSelection {
            items: vec![directive()],
        })
        .unwrap();
    assert!(engine.document().directives.is_empty());
    assert_eq!(connectivity(&engine), before, "removed");

    for step in ["delete", "move", "edit", "place"] {
        engine.undo().unwrap();
        assert_eq!(connectivity(&engine), before, "undo of the {step}");
    }
    assert!(engine.document().directives.is_empty());
}

/// A flag off every wire, or on a junction where two wires of one net meet,
/// joins nothing either.
#[test]
fn a_net_parameters_flag_off_a_wire_or_on_a_junction_joins_nothing() {
    for at in [Point::new(5.0, 5.0), Point::new(10.0, 0.0)] {
        let mut engine = Engine::new(sheet()).unwrap();
        let before = connectivity(&engine);
        engine
            .execute(Command::PlaceDirective {
                directive: net_parameters(at, Some("POWER"), NetRules::default()),
            })
            .unwrap();
        assert_eq!(connectivity(&engine), before, "{at:?}");
    }
}
