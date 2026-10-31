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
            let text = drawn_text_aabb(
                &label.text,
                None,
                label.font_size,
                (label.position.x, label.position.y),
                0.0,
                label.justify,
                label.justify_v,
            );
            assert!(
                !overlaps(text, glyph),
                "label {:?} {text:?} overlaps the {} port glyph {glyph:?}",
                label.text,
                port.value
            );
        }
    }
}

fn power_port(ref_text: Option<TextProp>) -> Symbol {
    Symbol {
        uuid: uuid::Uuid::now_v7(),
        reference: "#PWR?".into(),
        value: "+3V3".into(),
        is_power: true,
        ref_text,
        ..Symbol::default()
    }
}

fn reference_drawn(port: &Symbol) -> bool {
    snapshot::field_text_input(
        port,
        &port.reference,
        port.ref_text.as_ref(),
        snapshot::FieldSlot::Reference,
        [1.0; 4],
        None,
    )
    .is_some()
}

/// Place > Power Port builds the symbol with no reference prop.
#[test]
fn a_menu_placed_power_port_draws_no_designator() {
    assert!(!reference_drawn(&power_port(None)));
}

/// A port placed from a library row carries a visible reference prop; the
/// SVG/PDF export hides every power-port designator, so the canvas does too.
#[test]
fn a_library_placed_power_port_draws_no_designator() {
    let visible = TextProp {
        position: Point::new(0.0, -2.54),
        rotation: 0.0,
        font_size: 1.27,
        justify_h: HAlign::Center,
        justify_v: signex_types::schematic::VAlign::default(),
        hidden: false,
        font_family: None,
    };
    let port = power_port(Some(visible));
    assert!(!reference_drawn(&port));
    let bounds = collect_item_bounds(&SchematicSheet::default(), &Default::default());
    assert!(bounds.is_empty());
    let sheet = SchematicSheet {
        symbols: vec![port],
        ..SchematicSheet::default()
    };
    let has_ref_box = collect_item_bounds(&sheet, &Default::default())
        .iter()
        .any(|b| b.item.kind == signex_types::schematic::SelectedKind::SymbolRefField);
    assert!(!has_ref_box, "an undrawn designator must not be selectable");
}

#[test]
fn sample_netlist_names_the_three_rails() {
    let sheet = sample();
    let netlist = signex_net::build_netlist(&sheet);
    let mut names: Vec<_> = netlist.nets.iter().map(|n| n.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["+3V3", "+5V", "GND", "N$4"]);
}
