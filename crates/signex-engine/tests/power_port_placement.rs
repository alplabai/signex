//! A power port placed from the Active Bar carries a library entry with a
//! connection pin at its anchor (#1408), so the netlist and ERC see it, and
//! the whole placement is one undo step.

use signex_engine::{Engine, place_power_port};
use signex_erc::engine::run_all;
use signex_erc::{ErcContext, RuleId, RuleKind};
use signex_types::schematic::{Point, SchematicSheet, Symbol, Wire};
use uuid::Uuid;

fn port(lib_id: &str, value: &str, at: Point) -> Symbol {
    Symbol {
        uuid: Uuid::new_v4(),
        lib_id: lib_id.to_string(),
        value: value.to_string(),
        is_power: true,
        unit: 1,
        position: at,
        ..Default::default()
    }
}

fn wire(a: Point, b: Point) -> Wire {
    Wire {
        uuid: Uuid::new_v4(),
        start: a,
        end: b,
        ..Default::default()
    }
}

fn sheet_with_wires(wires: Vec<Wire>) -> Engine {
    Engine::new(SchematicSheet {
        paper_size: "A4".to_string(),
        wires,
        ..Default::default()
    })
    .expect("engine")
}

#[test]
fn placement_is_one_undo_step_and_keeps_the_style_in_the_lib_id() {
    let mut engine = sheet_with_wires(vec![]);
    engine
        .execute_batch(place_power_port(port(
            "power:GNDREF",
            "GNDREF",
            Point::new(0.0, 0.0),
        )))
        .expect("place");

    let doc = engine.document();
    assert_eq!(doc.symbols[0].lib_id, "power:GNDREF");
    let lib = doc.lib_symbols.get("power:GNDREF").expect("entry");
    assert_eq!(lib.pins.len(), 1);
    assert_eq!(
        (lib.pins[0].pin.position.x, lib.pins[0].pin.position.y),
        (0.0, 0.0)
    );

    engine.undo().expect("undo");
    assert!(engine.document().symbols.is_empty());
    assert!(engine.document().lib_symbols.is_empty());
}

#[test]
fn same_value_ports_on_disjoint_wires_join_by_name() {
    let mut engine = sheet_with_wires(vec![
        wire(Point::new(0.0, 0.0), Point::new(10.0, 0.0)),
        wire(Point::new(0.0, 20.0), Point::new(10.0, 20.0)),
    ]);
    for at in [Point::new(10.0, 0.0), Point::new(10.0, 20.0)] {
        engine
            .execute_batch(place_power_port(port("power:GND", "GND", at)))
            .expect("place");
    }
    let netlist = signex_net::build_netlist(engine.document());
    assert!(
        netlist.nets.iter().any(|n| n.name == "GND"),
        "a net named GND: {netlist:?}"
    );
    assert_eq!(netlist.nets.len(), 1, "{netlist:?}");
}

#[test]
fn erc_sees_the_pin_at_the_wire_end() {
    let mut engine = sheet_with_wires(vec![wire(Point::new(0.0, 0.0), Point::new(10.0, 0.0))]);
    let dangling_at_end = |engine: &Engine| {
        run_all(&ErcContext::from_snapshot(engine.document()))
            .iter()
            .filter(|d| {
                d.rule_id == RuleId::builtin(RuleKind::DanglingWire)
                    && (d.location.x - 10.0).abs() < 1e-6
                    && d.location.y.abs() < 1e-6
            })
            .count()
    };
    assert_eq!(dangling_at_end(&engine), 1);
    engine
        .execute_batch(place_power_port(port(
            "power:+5V",
            "+5V",
            Point::new(10.0, 0.0),
        )))
        .expect("place");
    assert_eq!(dangling_at_end(&engine), 0);
}
