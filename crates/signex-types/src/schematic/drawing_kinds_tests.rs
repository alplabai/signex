//! The Bezier, dimension and table drawing kinds (#1267, #1300): serde
//! tags, defaults and skip rules, dimension text, table layout and the
//! format 1.3 trigger.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::*;
use crate::net_class::NetLineStyle;

#[derive(Debug, Serialize, Deserialize)]
struct Doc {
    drawings: Vec<SchDrawing>,
}

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn to_toml(drawing: &SchDrawing) -> String {
    toml::to_string(&Doc {
        drawings: vec![drawing.clone()],
    })
    .expect("serialises")
}

fn from_toml(text: &str) -> SchDrawing {
    let mut doc: Doc = toml::from_str(text).expect("parses");
    assert_eq!(doc.drawings.len(), 1);
    doc.drawings.remove(0)
}

fn bezier() -> SchBezier {
    SchBezier::new(
        id(1),
        Point::new(8.89, 38.1),
        Point::new(10.16, 76.2),
        Point::new(83.82, 76.2),
        Point::new(87.63, 38.1),
    )
}

fn dimension(direction: DimensionDirection) -> SchDimension {
    SchDimension::new(
        id(2),
        Point::new(132.08, 25.4),
        Point::new(144.78, 25.4),
        -5.08,
        direction,
    )
}

#[test]
fn bezier_round_trips_with_its_tag_and_writes_no_defaults() {
    let drawing = SchDrawing::Bezier(bezier());
    let text = to_toml(&drawing);
    assert!(text.contains("type = \"bezier\""), "{text}");
    assert!(
        text.contains("c1"),
        "the struct's keys sit beside the tag: {text}"
    );
    assert!(!text.contains("line_style"), "solid is not written: {text}");
    assert!(
        !text.contains("ends"),
        "no arrowheads is not written: {text}"
    );
    assert!(
        !text.contains("locked"),
        "an open lock is not written: {text}"
    );
    assert_eq!(from_toml(&text), drawing);
}

#[test]
fn bezier_style_and_ends_are_written_when_set() {
    let mut b = bezier();
    b.line_style = NetLineStyle::Dashed;
    b.ends = LineEnds::Both;
    let drawing = SchDrawing::Bezier(b);
    let text = to_toml(&drawing);
    assert!(text.contains("line_style = \"dashed\""), "{text}");
    assert!(text.contains("ends = \"both\""), "{text}");
    assert_eq!(from_toml(&text), drawing);
}

#[test]
fn bezier_reads_with_only_its_points() {
    let text = r#"
[[drawings]]
type = "bezier"
uuid = "00000000-0000-0000-0000-000000000001"
start = { x = 0.0, y = 0.0 }
c1 = { x = 1.0, y = 2.0 }
c2 = { x = 3.0, y = 2.0 }
end = { x = 4.0, y = 0.0 }
"#;
    let SchDrawing::Bezier(b) = from_toml(text) else {
        panic!("a bezier");
    };
    assert_eq!(b.width, 0.0);
    assert_eq!(b.stroke_color, None, "the sheet's drawing colour, not red");
    assert_eq!(b.line_style, NetLineStyle::Solid);
    assert_eq!(b.ends, LineEnds::None);
    assert!(!b.locked);
}

#[test]
fn bezier_flattening_keeps_both_ends_exact() {
    let b = bezier();
    let points = b.flatten();
    assert_eq!(points.len(), BEZIER_FLATTEN_SEGMENTS + 1);
    assert_eq!(points[0], b.start);
    assert_eq!(*points.last().unwrap(), b.end);
    assert!(b.distance_to(b.point_at(0.5)) < 0.05);
}

#[test]
fn dimension_round_trips_and_defaults_read_back() {
    let drawing = SchDrawing::Dimension(dimension(DimensionDirection::Horizontal));
    let text = to_toml(&drawing);
    assert!(text.contains("type = \"dimension\""), "{text}");
    assert!(text.contains("direction = \"horizontal\""), "{text}");
    assert!(!text.contains("override_text"), "{text}");
    assert_eq!(from_toml(&text), drawing);

    let bare = r#"
[[drawings]]
type = "dimension"
uuid = "00000000-0000-0000-0000-000000000002"
from = { x = 0.0, y = 0.0 }
to = { x = 10.0, y = 0.0 }
offset = -2.0
direction = "aligned"
"#;
    let SchDrawing::Dimension(d) = from_toml(bare) else {
        panic!("a dimension");
    };
    assert_eq!(d.units, DimensionUnits::Mm);
    assert_eq!(d.precision, 2);
    assert_eq!(d.arrows, DimensionArrows::FilledInside);
    assert_eq!(d.override_text, None);
}

#[test]
fn dimension_label_follows_units_precision_and_override() {
    let mut d = dimension(DimensionDirection::Horizontal);
    assert!((d.measured_mm() - 12.7).abs() < 1e-9);
    assert_eq!(d.label(), "12.70 mm");
    d.precision = 0;
    assert_eq!(d.label(), "13 mm");
    d.precision = 3;
    d.units = DimensionUnits::In;
    assert_eq!(d.label(), "0.500 in");
    d.units = DimensionUnits::Mil;
    d.precision = 1;
    assert_eq!(d.label(), "500.0 mil");
    d.precision = 9;
    assert_eq!(d.label(), "500.000 mil", "precision stops at three");
    d.override_text = Some("12.70 typ.".into());
    assert_eq!(d.label(), "12.70 typ.");
    assert_eq!(d.measured_text(), "500.000 mil");
}

#[test]
fn dimension_direction_picks_the_measured_span() {
    let mut d = dimension(DimensionDirection::Vertical);
    d.to = Point::new(144.78, 30.48);
    assert!((d.measured_mm() - 5.08).abs() < 1e-9);
    d.direction = DimensionDirection::Aligned;
    assert!((d.measured_mm() - 12.7f64.hypot(5.08)).abs() < 1e-9);
}

#[test]
fn horizontal_dimension_line_sits_offset_from_the_points() {
    let d = dimension(DimensionDirection::Horizontal);
    let g = d.geometry();
    assert!((g.line[0].y - (25.4 - 5.08)).abs() < 1e-9);
    assert!((g.line[1].x - 144.78).abs() < 1e-9);
    assert_eq!(g.text_angle_deg, 0.0);
    assert!(
        g.text_at.y < g.line[0].y,
        "the text sits above a line drawn above"
    );
    assert!(!g.text_hangs);
}

#[test]
fn the_third_click_sets_the_offset_and_the_drag_sets_a_linear_direction() {
    let (a, b) = (Point::new(0.0, 0.0), Point::new(10.0, 2.0));
    assert_eq!(
        SchDimension::linear_direction(a, b),
        DimensionDirection::Horizontal
    );
    assert_eq!(
        SchDimension::linear_direction(a, Point::new(2.0, 10.0)),
        DimensionDirection::Vertical
    );
    let mut d = SchDimension::new(id(3), a, b, 0.0, DimensionDirection::Horizontal);
    d.offset = d.offset_to(Point::new(5.0, -4.0));
    assert!((d.offset + 4.0).abs() < 1e-9);
    assert!(
        (d.line()[0].y + 4.0).abs() < 1e-9,
        "the line runs through the click"
    );
    let mut aligned = SchDimension::new(
        id(4),
        a,
        Point::new(10.0, 0.0),
        0.0,
        DimensionDirection::Aligned,
    );
    aligned.offset = aligned.offset_to(Point::new(3.0, 2.5));
    assert!((aligned.line()[1].y - 2.5).abs() < 1e-9);
}

#[test]
fn quarter_turn_makes_a_horizontal_dimension_vertical() {
    let mut d = dimension(DimensionDirection::Horizontal);
    let before = d.measured_mm();
    let line_before = d.line();
    // A clockwise quarter turn about the origin in y-down space.
    let turn = |p: Point| Point::new(-p.y, p.x);
    d.map_points(turn);
    assert_eq!(d.direction, DimensionDirection::Vertical);
    assert!((d.measured_mm() - before).abs() < 1e-9);
    let line_after = d.line();
    let expected = turn(line_before[0]);
    assert!((line_after[0].x - expected.x).abs() < 1e-9);
    assert!((line_after[0].y - expected.y).abs() < 1e-9);
}

#[test]
fn move_keeps_a_dimension_unchanged_but_its_place() {
    let mut d = dimension(DimensionDirection::Aligned);
    let before = d.clone();
    d.map_points(|p| Point::new(p.x + 2.54, p.y - 1.27));
    assert_eq!(d.direction, before.direction);
    assert!((d.offset - before.offset).abs() < 1e-9);
    assert!((d.from.x - before.from.x - 2.54).abs() < 1e-9);
}

#[test]
fn table_round_trips_with_its_source_and_hidden_columns() {
    let mut table = SchTable::new_typed(id(3), Point::new(50.8, 25.4));
    table.source = TableSource::ComponentPins {
        designator: "U1".into(),
        sort: PinSort::Name,
    };
    table.columns = SchTable::columns_for(&table.source);
    table.rows = vec![vec!["1".into(), "BOOT".into(), "BOOT".into()]];
    table.links = vec![String::new()];
    table.title = Some("U1 pin assignment".into());
    let drawing = SchDrawing::Table(table);
    let text = to_toml(&drawing);
    assert!(text.contains("type = \"table\""), "{text}");
    assert!(text.contains("kind = \"component_pins\""), "{text}");
    assert!(
        text.contains("shown = false"),
        "hidden columns are written: {text}"
    );
    assert!(
        !text.contains("shown = true"),
        "shown is the default: {text}"
    );
    assert_eq!(from_toml(&text), drawing);
}

#[test]
fn table_style_and_source_defaults_read_back() {
    let text = r#"
[[drawings]]
type = "table"
uuid = "00000000-0000-0000-0000-000000000003"
position = { x = 0.0, y = 0.0 }
columns = [{ key = "page", title = "Page" }]
source = { kind = "project_sheets" }
style = {}
"#;
    let SchDrawing::Table(t) = from_toml(text) else {
        panic!("a table");
    };
    assert_eq!(t.source, TableSource::ProjectSheets { per_channel: true });
    assert!(t.columns[0].shown);
    assert_eq!(t.style, TableStyle::default());
    assert_eq!(t.style.text_size, 1.8);
    assert_eq!(t.style.cell_padding, 0.6);
    assert_eq!(t.refresh, TableRefresh::OnCompile);
    assert!(t.rows.is_empty() && t.links.is_empty());
}

#[test]
fn new_table_is_typed_three_by_three() {
    let t = SchTable::new_typed(id(4), Point::ZERO);
    assert_eq!(t.source, TableSource::Typed);
    assert_eq!(t.size(), (3, 3));
    assert!(!t.source.is_bound());
}

#[test]
fn table_layout_finds_cells_and_hides_unchecked_columns() {
    let mut t = SchTable::new_typed(id(5), Point::new(10.0, 10.0));
    t.set_cell(0, 0, "0x00".into());
    let layout = t.layout();
    assert_eq!(layout.columns, vec![0, 1, 2]);
    let row_h = 1.8 + 2.0 * 0.6;
    assert!((layout.row_height - row_h).abs() < 1e-9);
    let inside_first_body = Point::new(10.5, 10.0 + row_h * 1.5);
    assert_eq!(
        t.cell_at(inside_first_body),
        Some(TableCell {
            row: Some(0),
            column: 0
        })
    );
    assert_eq!(
        t.cell_at(Point::new(10.5, 10.5)),
        Some(TableCell {
            row: None,
            column: 0
        })
    );
    assert_eq!(t.cell_at(Point::new(9.0, 10.5)), None);

    assert!(t.set_column_shown("c1", false));
    assert_eq!(t.layout().columns, vec![1, 2]);
    assert_eq!(t.size(), (3, 2));
    assert!(!t.set_column_shown("missing", false));
}

#[test]
fn table_moves_but_never_turns() {
    let mut t = SchTable::new_typed(id(6), Point::new(10.0, 10.0));
    let (lo, hi) = t.bounds();
    let (w, h) = (hi.x - lo.x, hi.y - lo.y);
    t.map_points(|p| Point::new(-p.y, p.x));
    let (lo2, hi2) = t.bounds();
    assert!(((hi2.x - lo2.x) - w).abs() < 1e-9, "the width stays");
    assert!(((hi2.y - lo2.y) - h).abs() < 1e-9, "the height stays");
}

#[test]
fn only_the_new_kinds_need_format_1_3() {
    let line = SchDrawing::Line {
        uuid: id(7),
        start: Point::ZERO,
        end: Point::new(1.0, 0.0),
        width: 0.0,
        stroke_color: None,
        locked: false,
    };
    assert!(!line.needs_format_1_3());
    assert!(SchDrawing::Bezier(bezier()).needs_format_1_3());
    assert!(SchDrawing::Dimension(dimension(DimensionDirection::Horizontal)).needs_format_1_3());
    assert!(SchDrawing::Table(SchTable::new_typed(id(8), Point::ZERO)).needs_format_1_3());
}

#[test]
fn lock_and_id_cover_the_new_kinds() {
    for mut d in [
        SchDrawing::Bezier(bezier()),
        SchDrawing::Dimension(dimension(DimensionDirection::Horizontal)),
        SchDrawing::Table(SchTable::new_typed(id(9), Point::ZERO)),
    ] {
        assert!(!d.is_locked());
        d.set_locked(true);
        assert!(d.is_locked());
        d.set_uuid(id(42));
        assert_eq!(d.uuid(), id(42));
    }
}

#[test]
fn power_nets_come_from_the_sheet_power_ports() {
    let sheet = SchematicSheet::default();
    assert!(power_net_names(&sheet).is_empty());
}
