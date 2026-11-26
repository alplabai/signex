//! #1116: a placed image's geometry and its first size.

use uuid::Uuid;

use super::*;

fn image(at: Point, size: (f64, f64)) -> SchImage {
    SchImage::new(
        Uuid::from_u128(1),
        at,
        size,
        "assets/images/logo.svg".into(),
        "aa".into(),
    )
}

#[test]
fn an_image_turned_or_mirrored_keeps_upright_and_moves_its_centre() {
    let mut img = image(Point::new(10.0, 20.0), (40.0, 10.0));
    let mut drawing = SchDrawing::Image(img.clone());
    // A quarter turn about the origin: (x, y) -> (-y, x).
    assert!(drawing.map_annotation_points(|p| Point::new(-p.y, p.x)));
    let SchDrawing::Image(turned) = drawing else {
        panic!("still an image");
    };
    assert_eq!((turned.width, turned.height), (40.0, 10.0));
    assert_eq!(turned.centre(), Point::new(-25.0, 30.0));
    img.map_points(|p| Point::new(p.x + 1.0, p.y));
    assert_eq!(img.position, Point::new(11.0, 20.0));
}

#[test]
fn the_first_size_fills_the_free_area_keeping_the_aspect_ratio() {
    // A 1200 × 584 px file is 317.5 mm wide at 96 dpi; a 106.68 mm wide strip
    // leaves 96.52 mm inside its insets, on the 2.54 mm grid.
    let (w, h) = first_size_mm((317.5, 154.52), (106.68, 200.0));
    assert!((w - 96.52).abs() < 1e-9, "{w}");
    assert!((h - 96.52 * 154.52 / 317.5).abs() < 1e-9, "{h}");
}

#[test]
fn with_no_free_room_an_image_takes_its_own_size() {
    assert_eq!(first_size_mm((30.0, 20.0), (12.0, 400.0)), (30.0, 20.0));
}

#[test]
fn the_free_area_is_the_page_on_an_empty_sheet_and_a_strip_beside_content() {
    let mut sheet = SchematicSheet {
        paper_size: "A4".into(),
        ..Default::default()
    };
    assert_eq!(free_area_mm(&sheet), (297.0, 210.0));
    sheet.drawings.push(SchDrawing::Image(image(
        Point::new(0.0, 0.0),
        (200.0, 50.0),
    )));
    // Right of the content: 97 × 210; below it: 297 × 160.
    assert_eq!(free_area_mm(&sheet), (297.0, 160.0));
}
