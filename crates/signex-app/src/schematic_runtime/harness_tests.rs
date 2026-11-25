//! #1280: harness drawing and picking on the canvas.

use super::*;
use signex_types::schematic::{HarnessConnector, HarnessEntry, HarnessWire};

fn sheet() -> SchematicSheet {
    let mut s = SchematicSheet {
        paper_size: "A4".into(),
        ..Default::default()
    };
    s.harness.connectors.push(HarnessConnector {
        uuid: uuid::Uuid::from_u128(1),
        harness_type: "SPI_BUS".into(),
        position: Point::new(20.0, 20.0),
        width: 10.16,
        entry_pitch: 2.54,
        rotation: 0.0,
        entries: vec![HarnessEntry {
            uuid: uuid::Uuid::from_u128(2),
            name: "SCK".into(),
        }],
        locked: false,
    });
    s.harness.wires.push(HarnessWire {
        uuid: uuid::Uuid::from_u128(3),
        start: Point::new(40.0, 22.54),
        end: Point::new(60.0, 22.54),
        locked: false,
    });
    s
}

#[test]
fn a_harness_wire_is_drawn_thicker_than_a_bus_in_the_harness_colour() {
    let s = sheet();
    let colors = signex_types::theme::canvas_colors(signex_types::theme::ThemeId::GraphiteLight);
    let (mut wires, mut polygons, mut texts) = (Vec::new(), Vec::new(), Vec::new());
    let transform = ScreenTransform {
        offset_x: 0.0,
        offset_y: 0.0,
        scale: 3.0,
        device_px_per_logical_px: 1.0,
    };
    push(
        &s,
        &transform,
        &colors,
        Rectangle::new(iced::Point::ORIGIN, iced::Size::new(2000.0, 2000.0)),
        None,
        HarnessOut {
            wires: &mut wires,
            polygons: &mut polygons,
            texts: &mut texts,
        },
    );
    assert_eq!(wires.len(), 1);
    assert!(wires[0].width_mm > signex_types::schematic::SCHEMATIC_RENDER_BUS_STROKE_MM as f32);
    assert_eq!(
        wires[0].explicit_color,
        Some(to_rgba(to_iced(&colors.harness_color())))
    );
    // The body and one entry tab; the entry name and the type name.
    assert_eq!(polygons.len(), 2);
    let names: Vec<&str> = texts.iter().map(|t| t.content.as_str()).collect();
    assert_eq!(names, ["SCK", "SPI_BUS"]);
}

#[test]
fn every_harness_object_has_a_selection_box() {
    let mut out = Vec::new();
    push_item_bounds(&sheet(), &mut out);
    let kinds: Vec<SelectedKind> = out.iter().map(|b| b.item.kind).collect();
    assert_eq!(
        kinds,
        [
            SelectedKind::HarnessConnector,
            SelectedKind::HarnessEntry,
            SelectedKind::HarnessWire
        ]
    );
}

#[test]
fn a_click_on_the_harness_wire_picks_it() {
    let s = sheet();
    let (a, b, tolerance) = wire_segment(&s, uuid::Uuid::from_u128(3)).expect("wire");
    assert_eq!((a, b), (Point::new(40.0, 22.54), Point::new(60.0, 22.54)));
    assert!(tolerance >= signex_types::schematic::SCHEMATIC_HIT_HARNESS_TOLERANCE_MM);
}
