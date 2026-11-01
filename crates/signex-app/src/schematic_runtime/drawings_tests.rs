//! Drawings reach the scene as one ordered bucket, in file order (#1579).

use super::tests::empty_snapshot;
use super::*;
use signex_gfx::primitive::ordered::OrderedItem;
use signex_types::theme::{ThemeId, canvas_colors};

const SCALE: f32 = 4.0;

fn scene_of(drawings: Vec<SchDrawing>) -> Scene {
    let mut sheet = empty_snapshot();
    sheet.drawings = drawings;
    let transform = ScreenTransform {
        offset_x: 0.0,
        offset_y: 0.0,
        scale: SCALE,
        device_px_per_logical_px: signex_types::schematic::UNSCALED_DEVICE_PX_PER_LOGICAL_PX,
    };
    let bounds = Rectangle::new(iced::Point::ORIGIN, iced::Size::new(800.0, 600.0));
    build_schematic_scene(
        &sheet,
        &transform,
        &canvas_colors(ThemeId::Signex),
        bounds,
        None,
        SceneOverrides::default(),
    )
    .expect("drawings draw something")
}

fn line(x: f64) -> SchDrawing {
    SchDrawing::Line {
        uuid: uuid::Uuid::now_v7(),
        start: Point::new(x, 10.0),
        end: Point::new(x + 5.0, 10.0),
        width: 0.25,
        stroke_color: None,
        locked: false,
    }
}

fn rect(fill: FillType) -> SchDrawing {
    SchDrawing::Rect {
        uuid: uuid::Uuid::now_v7(),
        start: Point::new(10.0, 8.0),
        end: Point::new(20.0, 14.0),
        width: 0.25,
        fill,
        stroke_color: None,
        locked: false,
    }
}

fn circle(radius: f64, fill: FillType) -> SchDrawing {
    SchDrawing::Circle {
        uuid: uuid::Uuid::now_v7(),
        center: Point::new(30.0, 30.0),
        radius,
        width: 0.25,
        fill,
        stroke_color: None,
        locked: false,
    }
}

fn kind(item: &OrderedItem) -> &'static str {
    match item {
        OrderedItem::Line(_) => "line",
        OrderedItem::Circle(_) => "circle",
        OrderedItem::Arc(_) => "arc",
        OrderedItem::Polygon(_) => "polygon",
    }
}

#[test]
fn drawings_keep_file_order_across_kinds() {
    let scene = scene_of(vec![
        line(1.0),
        rect(FillType::None),
        line(2.0),
        circle(3.0, FillType::None),
    ]);

    let kinds: Vec<_> = scene.drawings.iter().map(kind).collect();
    assert_eq!(kinds, ["line", "polygon", "line", "circle"]);
    assert!(
        scene.lines.is_empty() && scene.polygons.is_empty() && scene.arcs.is_empty(),
        "no drawing is routed into a per-kind bucket"
    );
}

#[test]
fn an_arc_drawing_is_one_arc_item_and_a_collinear_one_two_lines() {
    let arc = |mid_y: f64| SchDrawing::Arc {
        uuid: uuid::Uuid::now_v7(),
        start: Point::new(0.0, 0.0),
        mid: Point::new(5.0, mid_y),
        end: Point::new(10.0, 0.0),
        width: 0.25,
        fill: FillType::None,
        stroke_color: None,
        locked: false,
    };

    let bent = scene_of(vec![arc(3.0)]);
    assert_eq!(bent.drawings.iter().map(kind).collect::<Vec<_>>(), ["arc"]);

    let straight = scene_of(vec![arc(0.0)]);
    assert_eq!(
        straight.drawings.iter().map(kind).collect::<Vec<_>>(),
        ["line", "line"]
    );
}

#[test]
fn an_unfilled_polyline_is_its_segments_and_a_filled_one_a_polygon() {
    let polyline = |fill| SchDrawing::Polyline {
        uuid: uuid::Uuid::now_v7(),
        points: vec![
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(4.0, 4.0),
        ],
        width: 0.25,
        fill,
        stroke_color: None,
        locked: false,
    };

    let open = scene_of(vec![polyline(FillType::None)]);
    assert_eq!(
        open.drawings.iter().map(kind).collect::<Vec<_>>(),
        ["line", "line"]
    );

    let filled = scene_of(vec![polyline(FillType::Background)]);
    assert_eq!(
        filled.drawings.iter().map(kind).collect::<Vec<_>>(),
        ["polygon"]
    );
}

/// A drawing circle is a fill disc, then a ring: separate fill and stroke
/// colours, no 40-gon.
#[test]
fn a_filled_circle_is_a_fill_disc_then_a_ring_with_separate_colours() {
    let scene = scene_of(vec![circle(3.0, FillType::Background)]);

    let [OrderedItem::Circle(disc), OrderedItem::Circle(ring)] = &scene.drawings[..] else {
        panic!("expected a disc and a ring, got {:?}", scene.drawings);
    };
    assert_eq!(disc.stroke_width, 0.0, "the disc is a fill");
    assert!(ring.stroke_width > 0.0, "the ring is a stroke");
    assert_eq!(disc.radius, ring.radius);
    assert_ne!(
        disc.color, ring.color,
        "fill and stroke keep their own colours"
    );
}

#[test]
fn an_unfilled_circle_is_a_ring_only() {
    let scene = scene_of(vec![circle(3.0, FillType::None)]);

    let [OrderedItem::Circle(ring)] = &scene.drawings[..] else {
        panic!("expected one ring, got {:?}", scene.drawings);
    };
    assert!(ring.stroke_width > 0.0);
}

/// The radius floor survives the move to circle instances: a sub-pixel
/// drawing circle is still the floor's radius.
#[test]
fn a_sub_pixel_circle_keeps_the_radius_floor() {
    let scene = scene_of(vec![circle(0.001, FillType::None)]);

    let OrderedItem::Circle(ring) = &scene.drawings[0] else {
        panic!("expected a ring");
    };
    let floor = screen_px_to_world_mm(
        signex_types::schematic::SCHEMATIC_RENDER_DRAWING_MIN_CIRCLE_RADIUS_PX,
        SCALE,
    ) as f32;
    assert!(
        ring.radius >= floor,
        "radius {} is under the floor {floor}",
        ring.radius
    );
}

/// A drawing placed above the circuit goes to its own bucket, and each
/// bucket keeps file order among its own drawings.
#[test]
fn above_drawings_go_to_the_front_bucket_in_file_order() {
    let (a, b, c) = (line(1.0), rect(FillType::None), line(2.0));
    let mut sheet = empty_snapshot();
    sheet
        .drawing_placements
        .insert(b.uuid(), DrawingPlacement::Above);
    sheet.drawings = vec![a, b, c];
    let transform = ScreenTransform {
        offset_x: 0.0,
        offset_y: 0.0,
        scale: SCALE,
        device_px_per_logical_px: signex_types::schematic::UNSCALED_DEVICE_PX_PER_LOGICAL_PX,
    };
    let bounds = Rectangle::new(iced::Point::ORIGIN, iced::Size::new(800.0, 600.0));

    let scene = build_schematic_scene(
        &sheet,
        &transform,
        &canvas_colors(ThemeId::Signex),
        bounds,
        None,
        SceneOverrides::default(),
    )
    .expect("drawings draw something");

    assert_eq!(
        scene.drawings.iter().map(kind).collect::<Vec<_>>(),
        ["line", "line"]
    );
    assert_eq!(
        scene.drawings_front.iter().map(kind).collect::<Vec<_>>(),
        ["polygon"]
    );
}

/// The render cache refreshes only the collections a patch names; the drawings
/// patch must carry the placements with it, or an edit in the Properties
/// panel would not reach the canvas (#1583).
#[test]
fn a_drawings_refresh_copies_the_placements() {
    let mut src = empty_snapshot();
    let drawing = rect(FillType::None);
    src.drawing_placements
        .insert(drawing.uuid(), DrawingPlacement::Above);
    src.drawings = vec![drawing.clone()];
    let mut dst = empty_snapshot();

    copy_invalidated(&mut dst, &src, RenderInvalidation::DRAWINGS);

    assert_eq!(
        dst.drawing_placement(drawing.uuid()),
        DrawingPlacement::Above
    );
}
