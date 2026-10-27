//! A power port placed from the Active Bar carries a library entry with a
//! connection pin at its anchor (#1408), so the netlist and ERC see it, and
//! the whole placement is one undo step.

use signex_engine::{Engine, place_power_port, restyle_power_port};
use signex_erc::engine::run_all;
use signex_erc::{ErcContext, RuleId, RuleKind};
use signex_types::format::SnxSchematic;
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

#[test]
fn redo_after_placement_restores_the_port_and_its_entry() {
    let mut engine = sheet_with_wires(vec![]);
    engine
        .execute_batch(place_power_port(port(
            "power:GND",
            "GND",
            Point::new(0.0, 0.0),
        )))
        .expect("place");
    engine.undo().expect("undo");
    engine.redo().expect("redo");
    assert_eq!(engine.document().symbols.len(), 1);
    assert!(engine.document().lib_symbols.contains_key("power:GND"));
}

#[test]
fn restyle_is_one_undo_step_and_redoes() {
    let mut engine = sheet_with_wires(vec![]);
    let sym = port("power:VCC", "VCC", Point::new(0.0, 0.0));
    let id = sym.uuid;
    engine.execute_batch(place_power_port(sym)).expect("place");
    engine
        .execute_batch(restyle_power_port(id, "power:GND", 180.0))
        .expect("restyle");
    let doc = engine.document();
    assert_eq!(doc.symbols[0].lib_id, "power:GND");
    assert!(doc.lib_symbols.contains_key("power:GND"));

    engine.undo().expect("undo");
    let doc = engine.document();
    assert_eq!(doc.symbols[0].lib_id, "power:VCC");
    assert!(!doc.lib_symbols.contains_key("power:GND"));

    engine.redo().expect("redo");
    let doc = engine.document();
    assert_eq!(doc.symbols[0].lib_id, "power:GND");
    assert!(doc.lib_symbols.contains_key("power:GND"));
}

#[test]
fn the_registered_entry_survives_a_snxsch_write_and_parse() {
    let mut engine = sheet_with_wires(vec![]);
    engine
        .execute_batch(place_power_port(port(
            "power:VCC",
            "VCC",
            Point::new(0.0, 0.0),
        )))
        .expect("place");
    let text = SnxSchematic::new(engine.document().clone())
        .write_string()
        .expect("write");
    let back = SnxSchematic::parse(&text).expect("parse").sheet;
    let (got, want) = (
        back.lib_symbols.get("power:VCC").expect("entry kept"),
        engine
            .document()
            .lib_symbols
            .get("power:VCC")
            .expect("entry"),
    );
    assert_eq!(got.pins.len(), want.pins.len());
    assert_eq!(got.graphics.len(), want.graphics.len());
    assert_eq!(got.pins[0].pin.position, want.pins[0].pin.position);
}

/// A file saved before ports carried an entry: two `power:VCC` ports on
/// disjoint wires, no `lib_symbols`.
const ENTRYLESS: &str = r#"format = "snxsch/1"
schematic_id = "00000000-0000-0000-0000-000000000064"
version = 0
generator = ""
generator_version = ""
paper_size = "A4"
root_sheet_page = "1"

[sheets.components]
content = """
uuid                                  ref   library    pos_x  pos_y  rotation  value  mpn
00000000-0000-0000-0000-000000000001  #PWR1  power:VCC  10     0      0         VCC    ""
00000000-0000-0000-0000-000000000002  #PWR2  power:VCC  10     20     0         VCC    ""
"""

[sheets.wires]
content = """
uuid                                  net  start_x  start_y  end_x  end_y  stroke_width
00000000-0000-0000-0000-000000000011  ""   0        0        10     0      0
00000000-0000-0000-0000-000000000012  ""   0        20       10     20     0
"""

[sheets.junctions]
content = """
uuid  pos_x  pos_y  diameter
"""

[sheets.labels]
content = """
uuid  text  pos_x  pos_y  rotation  kind  shape  font_size  justify  justify_v
"""

[extras.symbols.00000000-0000-0000-0000-000000000001]
is_power = true
[extras.symbols.00000000-0000-0000-0000-000000000002]
is_power = true
"#;

#[test]
fn ports_with_no_entry_connect_in_netlist_and_erc() {
    let sheet = SnxSchematic::parse(ENTRYLESS).expect("parse").sheet;
    assert!(sheet.lib_symbols.is_empty());
    assert!(sheet.symbols.iter().all(|s| s.is_power));

    let netlist = signex_net::build_netlist(&sheet);
    assert_eq!(netlist.nets.len(), 1, "{netlist:?}");
    assert_eq!(netlist.nets[0].name, "VCC");

    let dangling_at_port = run_all(&ErcContext::from_snapshot(&sheet))
        .iter()
        .filter(|d| {
            d.rule_id == RuleId::builtin(RuleKind::DanglingWire)
                && (d.location.x - 10.0).abs() < 1e-6
        })
        .count();
    assert_eq!(dangling_at_port, 0);
}
