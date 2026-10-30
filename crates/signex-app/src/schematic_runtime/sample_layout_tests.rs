//! The bundled Sample Project is the first design a new user sees (#1771):
//! every power port carries its net name and no field text sits on a body.

use super::*;
use signex_types::format::SnxSchematic;

fn sample() -> SchematicSheet {
    SnxSchematic::parse(crate::sample_project::SHEET_SOURCE)
        .expect("sample parses")
        .sheet
}

fn overlaps(a: Aabb, b: Aabb) -> bool {
    a.min_x < b.max_x && a.max_x > b.min_x && a.min_y < b.max_y && a.max_y > b.min_y
}

#[test]
fn every_sample_power_port_shows_its_net_name_and_no_designator() {
    let sheet = sample();
    let ports: Vec<_> = sheet.symbols.iter().filter(|s| s.is_power).collect();
    assert_eq!(ports.len(), 3, "+5V, +3V3 and GND");
    for port in ports {
        let name = port.val_text.as_ref();
        assert!(
            name.is_some_and(|t| !t.hidden) && !port.value.is_empty(),
            "power port {} must show its net name",
            port.value
        );
        assert!(
            sheet
                .lib_symbols
                .get(&port.lib_id)
                .is_some_and(|lib| !lib.pins.is_empty()),
            "power port {} needs its pin or it joins no net by name",
            port.value
        );
        assert!(
            port.ref_text.as_ref().is_none_or(|t| t.hidden),
            "power port {} must not show a designator",
            port.value
        );
    }
}

#[test]
fn no_sample_field_text_overlaps_a_symbol_body() {
    let sheet = sample();
    for symbol in sheet.symbols.iter().filter(|s| !s.is_power) {
        let body = symbol::symbol_bounds(&sheet, symbol);
        let fields = [
            (&symbol.reference, symbol.ref_text.as_ref()),
            (&symbol.value, symbol.val_text.as_ref()),
        ];
        for (text, prop) in fields {
            let Some(prop) = prop.filter(|p| !p.hidden) else {
                continue;
            };
            let bbox = text_prop_aabb(text, prop, symbol, None);
            assert!(
                !overlaps(bbox, body),
                "{} field {text:?} {bbox:?} overlaps its body {body:?}",
                symbol.reference
            );
        }
    }
}

#[test]
fn no_sample_net_label_sits_on_a_power_port_glyph() {
    let sheet = sample();
    for port in sheet.symbols.iter().filter(|s| s.is_power) {
        let glyph = symbol::symbol_bounds(&sheet, port);
        for label in &sheet.labels {
            let width = label.text.chars().count() as f64 * 1.27 * 0.6;
            let text = Aabb::new(
                label.position.x,
                label.position.y - 1.27,
                label.position.x + width,
                label.position.y,
            );
            assert!(
                !overlaps(text, glyph),
                "label {:?} overlaps the {} port glyph",
                label.text,
                port.value
            );
        }
    }
}

#[test]
fn a_placed_power_port_without_a_ref_prop_draws_no_designator() {
    let port = Symbol {
        uuid: uuid::Uuid::now_v7(),
        reference: "#PWR?".into(),
        value: "+3V3".into(),
        is_power: true,
        ref_text: None,
        ..Symbol::default()
    };
    let drawn = snapshot::field_text_input(
        &port,
        &port.reference,
        port.ref_text.as_ref(),
        snapshot::FieldSlot::Reference,
        [1.0; 4],
        None,
    );
    assert!(drawn.is_none(), "#PWR? must not be drawn: {drawn:?}");
}

#[test]
fn sample_netlist_names_the_three_rails() {
    let sheet = sample();
    let netlist = signex_net::build_netlist(&sheet);
    let mut names: Vec<_> = netlist.nets.iter().map(|n| n.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["+3V3", "+5V", "GND", "N$4"]);
}
