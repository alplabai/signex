//! Harness lookups, connector turns and the names new objects take (#1280).

use super::*;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn connector(rotation: f64, names: &[&str]) -> HarnessConnector {
    HarnessConnector {
        uuid: id(1),
        harness_type: "SPI_BUS".into(),
        position: Point::new(10.0, 20.0),
        width: DEFAULT_HARNESS_CONNECTOR_WIDTH_MM,
        entry_pitch: 2.54,
        rotation,
        entries: names
            .iter()
            .enumerate()
            .map(|(i, n)| HarnessEntry {
                uuid: id(100 + i as u128),
                name: (*n).to_string(),
            })
            .collect(),
        locked: false,
    }
}

fn definition(name: &str, entries: &[&str]) -> HarnessDefinition {
    HarnessDefinition {
        name: name.into(),
        description: String::new(),
        entries: entries
            .iter()
            .map(|e| super::super::HarnessDefinitionEntry {
                name: (*e).to_string(),
                harness: None,
            })
            .collect(),
    }
}

fn near(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6
}

#[test]
fn an_entry_is_found_by_its_own_id() {
    let harness = SheetHarness {
        connectors: vec![connector(0.0, &["SCK", "MOSI"])],
        ..Default::default()
    };
    let (owner, index) = harness.entry(id(101)).unwrap();
    assert_eq!(owner.uuid, id(1));
    assert_eq!(index, 1);
    assert!(harness.holds_id(id(100)));
    assert!(!harness.holds_id(id(999)));
}

#[test]
fn the_outline_is_a_body_with_a_pointed_tip_inside_the_bounds() {
    for rotation in [0.0, 90.0, 180.0, 270.0] {
        let c = connector(rotation, &["A", "B", "C"]);
        let b = c.bounds().expand(1e-6);
        let outline = c.outline();
        assert_eq!(outline.len(), 5, "{rotation}");
        assert!(outline.iter().all(|p| b.contains(p.x, p.y)), "{rotation}");
        assert!(near(outline[2], c.tip_point()), "{rotation}: {outline:?}");
    }
}

#[test]
fn a_quarter_turn_keeps_the_body_and_moves_the_tip() {
    let c = connector(0.0, &["A", "B"]);
    let centre = c.centre();
    // Clockwise on a y-down page: (x, y) -> (-y, x) about the centre.
    let turn = |p: Point| Point::new(centre.x - (p.y - centre.y), centre.y + (p.x - centre.x));
    let turned = c.mapped(turn);
    assert!(near(turned.tip_point(), turn(c.tip_point())));
    assert!(near(turned.centre(), centre));
    assert_eq!(turned.entries, c.entries);
}

#[test]
fn a_mirror_along_the_tip_axis_reverses_the_entries() {
    let c = connector(0.0, &["A", "B", "C"]);
    let centre = c.centre();
    // Flip top for bottom: the tip stays where it is.
    let flip = |p: Point| Point::new(p.x, 2.0 * centre.y - p.y);
    let mirrored = c.mapped(flip);
    assert!(near(mirrored.tip_point(), c.tip_point()));
    let names: Vec<&str> = mirrored.entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["C", "B", "A"]);
}

#[test]
fn a_new_connector_takes_the_last_type_then_the_first_definition() {
    let defs = [definition("SPI_BUS", &["SCK"]), definition("UART", &["TX"])];
    assert_eq!(new_connector_type(Some("UART"), &defs), "UART");
    assert_eq!(new_connector_type(None, &defs), "SPI_BUS");
    assert_eq!(new_connector_type(Some("  "), &[]), FALLBACK_HARNESS_TYPE);
}

#[test]
fn a_new_connector_is_seeded_from_its_definition() {
    let defs = [definition("SPI_BUS", &["SCK", "MOSI", "MISO", "CS"])];
    let mut n = 0;
    let entries = seeded_entries("SPI_BUS", &defs, || {
        n += 1;
        id(n)
    });
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["SCK", "MOSI", "MISO", "CS"]);
    assert!(seeded_entries("NONE", &defs, Uuid::nil).is_empty());
}

#[test]
fn a_new_entry_takes_the_next_unused_definition_name_then_entry_n() {
    let defs = [definition("SPI_BUS", &["SCK", "MOSI"])];
    let c = connector(0.0, &["SCK"]);
    assert_eq!(next_entry_name(&c, &defs), "MOSI");
    let full = connector(0.0, &["SCK", "MOSI", "ENTRY_1"]);
    assert_eq!(next_entry_name(&full, &defs), "ENTRY_2");
    assert_eq!(next_entry_name(&connector(0.0, &[]), &[]), "ENTRY_1");
}

#[test]
fn a_click_picks_the_nearest_slot() {
    let c = connector(0.0, &["A", "B"]);
    assert_eq!(c.slot_at(c.entry_point(0)), 0);
    assert_eq!(c.slot_at(c.entry_point(2)), 2);
}

fn names(c: &HarnessConnector) -> Vec<&str> {
    c.entries.iter().map(|e| e.name.as_str()).collect()
}

/// A dragged entry slides along the flat side (docs/DECISIONS.md,
/// 2026-11-24): to the nearest slot, clamped to the side's ends, the others
/// closing up; a point off the side picks the slot level with it.
#[test]
fn an_entry_slides_to_the_nearest_slot_and_stays_on_the_side() {
    for rotation in [0.0, 90.0, 180.0, 270.0] {
        let mut c = connector(rotation, &["A", "B", "C", "D"]);
        let third = c.entry_point(2);
        assert!(c.slide_entry(0, third), "{rotation}");
        assert_eq!(names(&c), ["B", "C", "A", "D"], "{rotation}");

        // Far past the last slot: it stops at the end of the side.
        let mut end = connector(rotation, &["A", "B", "C", "D"]);
        let (first, last) = (end.entry_point(0), end.entry_point(3));
        let beyond = Point::new(
            last.x + (last.x - first.x) * 5.0,
            last.y + (last.y - first.y) * 5.0,
        );
        assert!(end.slide_entry(1, beyond), "{rotation}");
        assert_eq!(names(&end), ["A", "C", "D", "B"], "{rotation}");
    }
}

#[test]
fn an_entry_left_on_its_own_slot_does_not_change() {
    let mut c = connector(0.0, &["A", "B"]);
    let own = c.entry_point(1);
    assert!(!c.slide_entry(1, Point::new(own.x - 1.0, own.y + 0.5)));
    assert!(!c.slide_entry(5, own), "no such entry");
    assert_eq!(names(&c), ["A", "B"]);
}

#[test]
fn uses_count_every_connector_of_a_type() {
    let a = SheetHarness {
        connectors: vec![connector(0.0, &[]), connector(90.0, &[])],
        ..Default::default()
    };
    let b = SheetHarness::default();
    assert_eq!(harness_type_uses([&a, &b], "SPI_BUS"), 2);
    assert_eq!(harness_type_uses([&a, &b], "UART"), 0);
}
