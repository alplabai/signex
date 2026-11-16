use signex_types::pcb::Point;
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};

use super::*;
use crate::pcb::tests::board;

/// A DXF file from `body`, the pairs of an ENTITIES section, with the
/// header's `$INSUNITS` set to `units`.
fn dxf(units: i32, body: &str) -> String {
    format!(
        "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n{units}\n0\nENDSEC\n\
         0\nSECTION\n2\nENTITIES\n{body}0\nENDSEC\n0\nEOF\n"
    )
}

fn line(ax: f64, ay: f64, bx: f64, by: f64) -> String {
    format!("0\nLINE\n8\n0\n10\n{ax}\n20\n{ay}\n11\n{bx}\n21\n{by}\n")
}

fn close(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
}

const SILK: PcbLayer = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);

#[test]
fn lines_arcs_and_circles_parse_in_mm_from_inches() {
    let body = format!(
        "{}0\nCIRCLE\n10\n1\n20\n1\n40\n0.5\n0\nARC\n10\n0\n20\n0\n40\n1\n50\n0\n51\n90\n0\nTEXT\n1\nhi\n",
        line(0.0, 0.0, 1.0, 0.0)
    );
    let drawing = parse_dxf(&dxf(1, &body)).unwrap();
    assert_eq!(drawing.skipped, 1, "the text is skipped");
    assert_eq!(
        drawing.entities[0],
        DxfEntity::Line {
            a: Point::new(0.0, 0.0),
            b: Point::new(25.4, 0.0)
        }
    );
    assert_eq!(
        drawing.entities[1],
        DxfEntity::Circle {
            center: Point::new(25.4, 25.4),
            radius: 12.7
        }
    );
    assert!(matches!(drawing.entities[2], DxfEntity::Arc { radius, .. } if radius == 25.4));
}

#[test]
fn a_file_without_entities_or_with_bad_numbers_is_refused() {
    assert_eq!(parse_dxf("hello"), Err(DxfError::NotDxf));
    let no_entities = "0\nSECTION\n2\nHEADER\n0\nENDSEC\n0\nEOF\n";
    assert_eq!(parse_dxf(no_entities), Err(DxfError::NoEntities));
    let bad = dxf(4, "0\nLINE\n10\nnan\n20\n0\n11\n1\n21\n0\n");
    assert!(matches!(parse_dxf(&bad), Err(DxfError::BadNumber(_))));
    assert!(matches!(
        parse_dxf(&dxf(3, &line(0.0, 0.0, 1.0, 1.0))),
        Err(DxfError::UnsupportedUnits(_))
    ));
}

/// The file's y grows up from the board origin: a point at y = 2 lands two
/// mm above the origin, at a smaller model y.
#[test]
fn drawings_land_from_the_board_origin_with_y_up_on_the_chosen_layer() {
    let mut b = board();
    b.options.origin.at = Point::new(10.0, 50.0);
    let text = dxf(4, &line(0.0, 2.0, 3.0, 2.0));
    let (command, summary) = dxf_import_command(&b, &text, DxfTarget::Layer(SILK)).unwrap();
    assert_eq!(summary.graphics, 1);
    assert_eq!(summary.target, "TopSilkscreen");
    let BoardCommand::AddGraphics { graphics } = &command else {
        panic!("a layer import adds drawings: {command:?}");
    };
    assert_eq!(graphics[0].layer, "TopSilkscreen");
    assert!(close(graphics[0].start.unwrap(), Point::new(10.0, 48.0)));
    assert!(close(graphics[0].end.unwrap(), Point::new(13.0, 48.0)));
}

/// A counter-clockwise quarter arc of the file, flipped into the model's
/// y-down axes, keeps its shape: the board arc runs from the file's end to
/// its start.
#[test]
fn a_file_arc_keeps_its_side_after_the_flip() {
    let body = "0\nARC\n10\n0\n20\n0\n40\n1\n50\n0\n51\n90\n";
    let drawing = parse_dxf(&dxf(4, body)).unwrap();
    let graphics = board_graphics(&drawing, Default::default(), "TopSilkscreen");
    let arc = &graphics[0];
    assert_eq!(arc.graphic_type, "arc");
    assert!(close(arc.start.unwrap(), Point::new(0.0, -1.0)));
    assert!(close(arc.end.unwrap(), Point::new(1.0, 0.0)));
}

#[test]
fn a_bulged_polyline_segment_becomes_an_arc_and_a_closed_square_a_polygon() {
    let square =
        "0\nLWPOLYLINE\n90\n4\n70\n1\n10\n0\n20\n0\n10\n4\n20\n0\n10\n4\n20\n4\n10\n0\n20\n4\n";
    let bulged = "0\nLWPOLYLINE\n90\n2\n70\n0\n10\n1\n20\n0\n42\n0.41421356237\n10\n0\n20\n1\n";
    let drawing = parse_dxf(&dxf(4, &format!("{square}{bulged}"))).unwrap();
    let graphics = board_graphics(&drawing, Default::default(), "TopSilkscreen");
    assert_eq!(graphics.len(), 2);
    assert_eq!(graphics[0].graphic_type, "poly");
    assert_eq!(graphics[0].points.len(), 4);
    assert_eq!(graphics[1].graphic_type, "arc");
    let center = graphics[1].center.unwrap();
    assert!(center.x.abs() < 1e-6 && center.y.abs() < 1e-6, "{center:?}");
    assert!((graphics[1].radius - 1.0).abs() < 1e-6);
}

#[test]
fn four_joined_lines_become_the_outline_and_undo_restores_the_old_one() {
    let mut b = board();
    let old = b.outline.clone();
    let body = [
        line(0.0, 0.0, 20.0, 0.0),
        line(20.0, 10.0, 20.0, 0.0),
        line(20.0, 10.0, 0.0, 10.0),
        line(0.0, 10.0, 0.0, 0.0),
        line(50.0, 50.0, 51.0, 51.0),
    ]
    .concat();
    let (command, summary) = dxf_import_command(&b, &dxf(4, &body), DxfTarget::Outline).unwrap();
    assert_eq!(summary.outline_points, 4);
    let applied = command.apply(&mut b).unwrap();
    assert_eq!(b.outline.len(), 4);
    assert!(b.outline.iter().any(|p| close(*p, Point::new(20.0, -10.0))));
    applied.inverse.apply(&mut b).unwrap();
    assert_eq!(b.outline, old);
}

#[test]
fn an_outline_import_without_a_closed_shape_is_refused() {
    let b = board();
    let text = dxf(4, &line(0.0, 0.0, 5.0, 5.0));
    assert_eq!(
        dxf_import_command(&b, &text, DxfTarget::Outline).map(|(_, s)| s),
        Err(DxfError::NoClosedShape)
    );
}

#[test]
fn a_layer_import_is_one_step_that_undo_takes_back_whole() {
    let mut b = board();
    let before = b.graphics.len();
    let body = [line(0.0, 0.0, 1.0, 0.0), line(1.0, 0.0, 1.0, 1.0)].concat();
    let (command, _) = dxf_import_command(&b, &dxf(4, &body), DxfTarget::Layer(SILK)).unwrap();
    let applied = command.apply(&mut b).unwrap();
    assert_eq!(b.graphics.len(), before + 2);
    assert_eq!(
        applied.patch.kind,
        crate::pcb::BoardPatchKind::GraphicsChanged
    );
    let redo = applied.inverse.apply(&mut b).unwrap();
    assert_eq!(b.graphics.len(), before);
    redo.inverse.apply(&mut b).unwrap();
    assert_eq!(b.graphics.len(), before + 2);
}

/// A drawing on copper would be copper nothing electrical reads: refused
/// before the file is parsed.
#[test]
fn a_layer_import_onto_copper_is_refused() {
    use signex_types::pcb_layer::CopperLayer;
    let b = board();
    let top = PcbLayer::Copper(CopperLayer::Top);
    let text = dxf(4, &line(0.0, 0.0, 1.0, 0.0));
    assert_eq!(
        dxf_import_command(&b, &text, DxfTarget::Layer(top)).map(|(_, s)| s),
        Err(DxfError::CopperLayer("TopCopper".to_string()))
    );
    assert_eq!(check_dxf_target(DxfTarget::Layer(SILK)), Ok(()));
    assert_eq!(check_dxf_target(DxfTarget::Outline), Ok(()));
}

/// A finite coordinate past the board file's nanometre range would import
/// and then fail every save: refused at import.
#[test]
fn a_drawing_past_the_file_range_is_refused() {
    let b = board();
    let far = dxf(4, &line(0.0, 0.0, 1.0e20, 0.0));
    assert_eq!(
        dxf_import_command(&b, &far, DxfTarget::Layer(SILK)).map(|(_, s)| s),
        Err(DxfError::OutOfRange)
    );
    let square = [
        line(0.0, 0.0, 1.0e20, 0.0),
        line(1.0e20, 0.0, 1.0e20, 1.0e20),
        line(1.0e20, 1.0e20, 0.0, 1.0e20),
        line(0.0, 1.0e20, 0.0, 0.0),
    ]
    .concat();
    assert_eq!(
        dxf_import_command(&b, &dxf(4, &square), DxfTarget::Outline).map(|(_, s)| s),
        Err(DxfError::OutOfRange)
    );

    // The command refuses it too, whoever builds it.
    let mut b = board();
    let (command, _) = dxf_import_command(
        &b,
        &dxf(4, &line(0.0, 0.0, 1.0, 0.0)),
        DxfTarget::Layer(SILK),
    )
    .unwrap();
    let BoardCommand::AddGraphics { graphics } = &command else {
        panic!("a layer import adds drawings: {command:?}");
    };
    let mut graphics = graphics.clone();
    graphics[0].end = Some(Point::new(1.0e20, 0.0));
    let before = b.graphics.len();
    assert_eq!(
        BoardCommand::AddGraphics { graphics }
            .apply(&mut b)
            .map(|_| ()),
        Err(crate::pcb::BoardCommandError::NotStorable { field: "drawing" })
    );
    assert_eq!(b.graphics.len(), before);
}
