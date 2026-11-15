//! #2374: guide geometry, grid choice and pour names.

use super::*;

fn close(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
}

#[test]
fn a_point_projects_onto_each_kind_of_guide() {
    let at = Point::new(10.0, 5.0);
    let p = Point::new(13.0, 9.0);
    let guide = |kind| BoardGuide::new(Uuid::nil(), kind, at);
    assert!(close(
        guide(GuideKind::Vertical).nearest_point(p),
        Point::new(10.0, 9.0)
    ));
    assert!(close(
        guide(GuideKind::Horizontal).nearest_point(p),
        Point::new(13.0, 5.0)
    ));
    // Falling to the right: x - y stays 5.
    let q = guide(GuideKind::Minus45).nearest_point(p);
    assert!((q.x - q.y - 5.0).abs() < 1e-9, "{q:?}");
    // Rising to the right: x + y stays 15.
    let q = guide(GuideKind::Plus45).nearest_point(p);
    assert!((q.x + q.y - 15.0).abs() < 1e-9, "{q:?}");
}

#[test]
fn guides_cross_unless_parallel() {
    let v = BoardGuide::new(Uuid::nil(), GuideKind::Vertical, Point::new(3.0, 0.0));
    let h = BoardGuide::new(Uuid::nil(), GuideKind::Horizontal, Point::new(0.0, 7.0));
    assert!(close(v.crossing(&h).unwrap(), Point::new(3.0, 7.0)));
    let d = BoardGuide::new(Uuid::nil(), GuideKind::Minus45, Point::ZERO);
    assert!(close(d.crossing(&v).unwrap(), Point::new(3.0, 3.0)));
    let v2 = BoardGuide::new(Uuid::nil(), GuideKind::Vertical, Point::new(9.0, 0.0));
    assert_eq!(v.crossing(&v2), None);
}

#[test]
fn the_first_grid_that_applies_governs_else_the_board_grid() {
    let grid = |step: f64, components: bool| BoardGrid {
        uuid: Uuid::nil(),
        name: String::new(),
        step_mm: step,
        origin: Point::new(0.1, 0.2),
        color: MARK_COLORS[0],
        components,
        non_components: !components,
    };
    let options = BoardOptions {
        grids: vec![grid(0.5, true), grid(0.25, true)],
        ..BoardOptions::default()
    };
    let spec = options.grid_for(GridUse::Components, 1.27);
    assert_eq!(spec.step_mm, 0.5);
    assert!(close(spec.snap(Point::new(1.0, 1.0)), Point::new(1.1, 1.2)));
    let spec = options.grid_for(GridUse::NonComponents, 1.27);
    assert_eq!(spec.step_mm, 1.27);
    assert_eq!(spec.origin, Point::ZERO);
}

#[test]
fn pour_names_follow_the_scheme() {
    let name = |scheme: PolygonNaming| scheme.name("GND", 1, "Top", 2);
    assert_eq!(name(PolygonNaming::NetLayerPour), "GND_L01_P002");
    assert_eq!(name(PolygonNaming::LayerNetPour), "L01_GND_P002");
    assert_eq!(name(PolygonNaming::BoardLayerNetPour), "Top_GND_P002");
    assert_eq!(name(PolygonNaming::NetBoardLayerPour), "GND_Top_P002");
}

#[test]
fn swatches_step_round_the_list() {
    assert_eq!(next_mark_color(MARK_COLORS[0]), MARK_COLORS[1]);
    assert_eq!(next_mark_color(MARK_COLORS[5]), MARK_COLORS[0]);
    assert_eq!(next_mark_color(Color::new(1, 2, 3, 4)), MARK_COLORS[0]);
}
