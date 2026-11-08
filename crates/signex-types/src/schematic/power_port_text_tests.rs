//! Pins the 2026-11-07 decision: a power port's text is fixed to the port.

use super::*;
use crate::schematic::{field_effective_style, power_port_lib_symbol, power_port_value_field};

const AT: (f64, f64) = (50.8, 25.4);

fn port(lib_id: &str, rotation: f64, mirror_x: bool, mirror_y: bool) -> Symbol {
    let at = Point::new(AT.0, AT.1);
    Symbol {
        uuid: uuid::Uuid::now_v7(),
        lib_id: lib_id.to_owned(),
        value: "VCC".to_owned(),
        position: at,
        rotation,
        mirror_x,
        mirror_y,
        is_power: true,
        val_text: Some(power_port_value_field(lib_id, at)),
        ..Symbol::default()
    }
}

fn near(a: Point, x: f64, y: f64) -> bool {
    (a.x - x).abs() < 1e-9 && (a.y - y).abs() < 1e-9
}

/// Where and how the text is drawn: world anchor and the resolved
/// `(rotation, justify_h, justify_v)` every reader draws with.
fn drawn(sym: &Symbol) -> (Point, (f64, HAlign, VAlign)) {
    let text = power_port_value_text(&power_port_lib_symbol(&sym.lib_id), sym);
    (text.position, field_effective_style(&text, sym))
}

#[test]
fn an_unturned_port_draws_its_text_where_placement_puts_it() {
    for lib_id in ["power:VCC", "power:GND", "power:arrow", "power:Earth"] {
        let sym = port(lib_id, 0.0, false, false);
        let placed = power_port_value_field(lib_id, sym.position);
        let text = power_port_value_text(&power_port_lib_symbol(lib_id), &sym);
        assert!(
            near(text.position, placed.position.x, placed.position.y),
            "{lib_id}"
        );
        assert_eq!(text.justify_h, placed.justify_h, "{lib_id}");
        assert_eq!(text.justify_v, placed.justify_v, "{lib_id}");
        assert_eq!(text.rotation, 0.0, "{lib_id}");
    }
}

#[test]
fn a_stored_text_offset_is_ignored_but_size_face_and_visibility_are_kept() {
    let mut sym = port("power:VCC", 0.0, false, false);
    let fixed = drawn(&sym);
    let stored = sym.val_text.as_mut().expect("value field");
    stored.position = Point::new(0.0, 0.0);
    stored.rotation = 90.0;
    stored.justify_h = HAlign::Left;
    stored.font_size = 2.5;
    stored.font_family = Some("Iosevka".into());
    stored.hidden = true;
    let text = power_port_value_text(&power_port_lib_symbol(&sym.lib_id), &sym);
    assert_eq!(drawn(&sym), fixed, "the stored offset moves nothing");
    assert_eq!(text.font_size, 2.5);
    assert_eq!(text.font_family.as_deref(), Some("Iosevka"));
    assert!(text.hidden);
}

#[test]
fn the_text_moves_with_the_port() {
    let mut sym = port("power:VCC", 0.0, false, false);
    let (before, _) = drawn(&sym);
    sym.position.x += 7.62;
    sym.position.y -= 2.54;
    let (after, _) = drawn(&sym);
    assert!(near(after, before.x + 7.62, before.y - 2.54));
}

#[test]
fn the_text_follows_a_turn_and_stays_upright_away_from_the_head() {
    let (x, y) = AT;
    let reach = 2.54 + 0.254;
    // (rotation, expected anchor, expected justification)
    let cases = [
        (0.0, (x, y - reach), (HAlign::Center, VAlign::Bottom)),
        (90.0, (x - reach, y), (HAlign::Right, VAlign::Center)),
        (180.0, (x, y + reach), (HAlign::Center, VAlign::Top)),
        (270.0, (x + reach, y), (HAlign::Left, VAlign::Center)),
    ];
    for (rotation, (ax, ay), (h, v)) in cases {
        let (anchor, style) = drawn(&port("power:VCC", rotation, false, false));
        assert!(near(anchor, ax, ay), "{rotation}: {anchor:?}");
        assert_eq!(style, (0.0, h, v), "{rotation}");
    }
}

#[test]
fn the_text_follows_a_mirror() {
    let (x, y) = AT;
    let reach = 2.54 + 0.254;
    let (anchor, style) = drawn(&port("power:VCC", 0.0, true, false));
    assert!(near(anchor, x, y + reach), "{anchor:?}");
    assert_eq!(style, (0.0, HAlign::Center, VAlign::Top));
    let (anchor, style) = drawn(&port("power:VCC", 90.0, false, true));
    assert!(near(anchor, x + reach, y), "{anchor:?}");
    assert_eq!(style, (0.0, HAlign::Left, VAlign::Center));
}

#[test]
fn only_a_power_port_reads_the_fixed_text() {
    let mut sheet = SchematicSheet::default();
    let mut part = port("Device:R", 0.0, false, false);
    part.is_power = false;
    part.val_text.as_mut().expect("value field").position = Point::new(1.0, 2.0);
    let mut power = port("power:VCC", 0.0, false, false);
    power.val_text.as_mut().expect("value field").position = Point::new(1.0, 2.0);
    sheet.symbols = vec![part.clone(), power.clone()];
    let part_text = sheet.drawn_value_text(&part).expect("part text");
    assert!(matches!(part_text, Cow::Borrowed(_)));
    assert!(near(part_text.position, 1.0, 2.0));
    let power_text = sheet.drawn_value_text(&power).expect("port text");
    assert!(!near(power_text.position, 1.0, 2.0));
    power.val_text = None;
    assert!(
        sheet.drawn_value_text(&power).is_some(),
        "a port without a stored field still draws its net name"
    );
}

#[test]
fn a_power_port_field_selects_the_port() {
    let mut sheet = SchematicSheet::default();
    let power = port("power:VCC", 0.0, false, false);
    let mut part = power.clone();
    part.uuid = uuid::Uuid::now_v7();
    part.is_power = false;
    sheet.symbols = vec![power.clone(), part.clone()];
    let item = |uuid, kind| SelectedItem::new(uuid, kind);
    let port_body = item(power.uuid, SelectedKind::Symbol);
    let port_value = item(power.uuid, SelectedKind::SymbolValField);
    let port_reference = item(power.uuid, SelectedKind::SymbolRefField);
    let part_value = item(part.uuid, SelectedKind::SymbolValField);
    assert_eq!(sheet.selection_unit(port_value), port_body);
    assert_eq!(sheet.selection_unit(port_reference), port_body);
    assert_eq!(sheet.selection_unit(part_value), part_value);
    assert_eq!(
        sheet.selection_units(&[port_value, part_value, port_body, port_reference]),
        vec![port_body, part_value],
        "each unit once, in order"
    );
}
