//! The cached hover hit test answers exactly what `hit_test_styled` does.

use signex_types::schematic::{Bus, BusEntry, ChildSheet, NoConnect, Point, SchematicSheet};
use uuid::Uuid;

use super::render_bench_tests::{Variant, generate};
use super::*;
use crate::schematic_runtime::hit_test::hit_test_styled;
use crate::schematic_runtime::{Designators, SchematicRenderCache, WireWidths};

/// A generated sheet with items the generator leaves out, several overlapping
/// so the topmost-wins order is exercised.
fn sheet() -> SchematicSheet {
    let mut sheet = generate(6, 5, Variant::from_env()).sheet;
    let mut n = 1_000_000u128;
    let mut id = || {
        n += 1;
        Uuid::from_u128(n)
    };
    for k in 0..40 {
        let x = 30.0 + 7.0 * f64::from(k);
        sheet.buses.push(Bus {
            uuid: id(),
            start: Point::new(x, 25.0),
            end: Point::new(x + 40.0, 31.0 + f64::from(k)),
        });
        sheet.bus_entries.push(BusEntry {
            uuid: id(),
            position: Point::new(x, 40.0),
            size: (2.54, 2.54),
        });
        sheet.no_connects.push(NoConnect {
            uuid: id(),
            position: Point::new(x + 1.0, 41.0),
        });
    }
    sheet.child_sheets.push(ChildSheet {
        uuid: id(),
        name: "child".into(),
        filename: "child.snxsch".into(),
        position: Point::new(60.0, 20.0),
        size: (400.0, 90.0),
        stroke_width: 0.0,
        fill: Default::default(),
        stroke_color: None,
        pins: Vec::new(),
        ..serde_json::from_value(serde_json::json!({
            "uuid": Uuid::nil(), "name": "", "filename": "",
            "position": {"x": 0.0, "y": 0.0}, "size": [1.0, 1.0]
        }))
        .expect("child sheet")
    });
    sheet
}

/// Points over the sheet: a pseudo-random spread, plus every wire and bus end
/// and middle with the offsets that land on and around the tolerance edge.
fn points(sheet: &SchematicSheet) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    for _ in 0..4000 {
        out.push((next() * 400.0, next() * 220.0));
    }
    let offsets = [0.0, 0.1, 0.25, 0.26, -0.3, 0.9];
    let ends = sheet
        .wires
        .iter()
        .map(|w| (w.start, w.end))
        .chain(sheet.buses.iter().map(|b| (b.start, b.end)));
    for (a, b) in ends {
        for p in [a, b, Point::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5)] {
            for d in offsets {
                out.push((p.x + d, p.y - d * 0.5));
            }
        }
    }
    for s in &sheet.symbols {
        out.push((s.position.x, s.position.y));
    }
    for l in &sheet.labels {
        out.push((l.position.x, l.position.y));
    }
    out.push((f64::NAN, 1.0));
    out.push((1e12, -1e12));
    out
}

#[test]
fn the_cached_hit_test_matches_the_uncached_one_at_every_point() {
    // Arrange
    let sheet = sheet();
    let mut slot = CanvasSlot::new();
    slot.set_render_cache(Some(SchematicRenderCache::from_sheet_at(&sheet, 7)));
    slot.designators
        .insert(sheet.symbols[0].uuid, "U1A-LONG-DESIGNATOR".into());
    let mut hits = 0;

    // Act / Assert
    for (x, y) in points(&sheet) {
        let want = hit_test_styled(&sheet, &slot.designators, x, y, WireWidths::default());
        let got = slot.hit_item_at(x, y, WireWidths::default(), 0);
        assert_eq!(got, want, "at ({x}, {y})");
        hits += usize::from(want.is_some());
    }
    assert!(hits > 1000, "the points must land on items, got {hits}");
}

#[test]
fn the_cache_follows_the_revision() {
    // Arrange
    let mut sheet = sheet();
    let mut slot = CanvasSlot::new();
    slot.set_render_cache(Some(SchematicRenderCache::from_sheet_at(&sheet, 7)));
    let wire = sheet.wires[0].clone();
    let away = (
        (wire.start.x + wire.end.x) * 0.5,
        (wire.start.y + wire.end.y) * 0.5 + 500.0,
    );
    assert_eq!(
        slot.hit_item_at(away.0, away.1, WireWidths::default(), 0),
        None
    );

    // Act: an edit moves the wire there, which moves the revision.
    sheet.wires[0].start.y += 500.0;
    sheet.wires[0].end.y += 500.0;
    let render = slot.render_cache.as_mut().expect("render cache");
    render.update_from_sheet(
        &sheet,
        crate::schematic_runtime::RenderInvalidation::FULL,
        7,
        8,
    );

    // Assert
    let want = hit_test_styled(
        &sheet,
        &Designators::new(),
        away.0,
        away.1,
        WireWidths::default(),
    );
    assert_eq!(want.map(|i| i.uuid), Some(wire.uuid));
    assert_eq!(
        slot.hit_item_at(away.0, away.1, WireWidths::default(), 0),
        want
    );
}
