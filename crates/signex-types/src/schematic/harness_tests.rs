//! Harness connector geometry: entry and tip points and bounds for each
//! rotation, and the serde defaults of the harness types.

use super::*;

const EPS: f64 = 1e-9;

fn connector(rotation: f64) -> HarnessConnector {
    HarnessConnector {
        uuid: Uuid::from_u128(1),
        harness_type: "SPI_BUS".into(),
        position: Point::new(100.0, 50.0),
        width: 10.0,
        entry_pitch: 2.54,
        rotation,
        entries: vec![
            HarnessEntry {
                uuid: Uuid::from_u128(2),
                name: "SCK".into(),
            },
            HarnessEntry {
                uuid: Uuid::from_u128(3),
                name: "MOSI".into(),
            },
        ],
        locked: false,
    }
}

#[track_caller]
fn assert_point(got: Point, x: f64, y: f64) {
    assert!(
        (got.x - x).abs() < EPS && (got.y - y).abs() < EPS,
        "expected ({x}, {y}), got ({}, {})",
        got.x,
        got.y
    );
}

#[track_caller]
fn assert_bounds(got: Aabb, min_x: f64, min_y: f64, max_x: f64, max_y: f64) {
    assert_point(Point::new(got.min_x, got.min_y), min_x, min_y);
    assert_point(Point::new(got.max_x, got.max_y), max_x, max_y);
}

#[test]
fn height_is_one_pitch_more_than_the_entries() {
    assert!((connector(0.0).height() - 7.62).abs() < EPS);
    let mut empty = connector(0.0);
    empty.entries.clear();
    assert!((empty.height() - 2.54).abs() < EPS);
}

#[test]
fn unturned_entries_sit_on_the_left_and_the_tip_points_right() {
    let c = connector(0.0);
    assert_point(c.entry_point(0), 100.0, 52.54);
    assert_point(c.entry_point(1), 100.0, 55.08);
    assert_point(c.tip_point(), 112.54, 53.81);
    assert_bounds(c.bounds(), 100.0, 50.0, 112.54, 57.62);
}

#[test]
fn a_quarter_turn_puts_the_entries_on_the_bottom_and_the_tip_up() {
    let c = connector(90.0);
    assert_point(c.entry_point(0), 102.54, 60.0);
    assert_point(c.entry_point(1), 105.08, 60.0);
    assert_point(c.tip_point(), 103.81, 47.46);
    assert_bounds(c.bounds(), 100.0, 47.46, 107.62, 60.0);
}

#[test]
fn a_half_turn_puts_the_entries_on_the_right_and_the_tip_left() {
    let c = connector(180.0);
    assert_point(c.entry_point(0), 110.0, 55.08);
    assert_point(c.entry_point(1), 110.0, 52.54);
    assert_point(c.tip_point(), 97.46, 53.81);
    assert_bounds(c.bounds(), 97.46, 50.0, 110.0, 57.62);
}

#[test]
fn three_quarter_turns_put_the_entries_on_top_and_the_tip_down() {
    let c = connector(270.0);
    assert_point(c.entry_point(0), 105.08, 50.0);
    assert_point(c.entry_point(1), 102.54, 50.0);
    assert_point(c.tip_point(), 103.81, 62.54);
    assert_bounds(c.bounds(), 100.0, 50.0, 107.62, 62.54);
}

#[test]
fn a_negative_or_full_turn_is_the_same_as_its_quarter_turn_equivalent() {
    assert_eq!(connector(-90.0).tip_point(), connector(270.0).tip_point());
    assert_eq!(connector(360.0).tip_point(), connector(0.0).tip_point());
    // An off-axis angle rounds to the nearest quarter turn.
    assert_eq!(connector(89.0).tip_point(), connector(90.0).tip_point());
}

#[test]
fn a_connector_reads_with_its_defaults() {
    let c: HarnessConnector = serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(9),
        "harness_type": "UART",
        "position": { "x": 1.0, "y": 2.0 },
        "width": 5.08,
    }))
    .unwrap();
    assert_eq!(c.entry_pitch, DEFAULT_HARNESS_ENTRY_PITCH_MM);
    assert_eq!(c.rotation, 0.0);
    assert!(c.entries.is_empty());
    assert!(!c.locked);
    let json = serde_json::to_value(&c).unwrap();
    assert!(json.get("locked").is_none(), "{json}");
}

#[test]
fn an_empty_sheet_harness_is_empty() {
    assert!(SheetHarness::default().is_empty());
    let mut harness = SheetHarness::default();
    harness
        .entry_types
        .insert(Uuid::from_u128(4), "UART".into());
    assert!(!harness.is_empty());
}

#[test]
fn a_definition_entry_without_a_harness_is_a_signal() {
    let entry: HarnessDefinitionEntry =
        serde_json::from_value(serde_json::json!({ "name": "TX" })).unwrap();
    assert_eq!(entry.harness, None);
    let definition: HarnessDefinition =
        serde_json::from_value(serde_json::json!({ "name": "UART" })).unwrap();
    assert!(definition.entries.is_empty());
    assert!(definition.description.is_empty());
}
