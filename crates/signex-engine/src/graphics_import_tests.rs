use super::*;
use signex_types::object_id::SequentialIds;

/// A DXF file: an optional `$INSUNITS` header, then `entities` as written.
fn dxf(units: Option<i64>, entities: &str) -> String {
    let header = units
        .map(|u| format!("0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n{u}\n0\nENDSEC\n"))
        .unwrap_or_default();
    format!("{header}0\nSECTION\n2\nENTITIES\n{entities}0\nENDSEC\n0\nEOF\n")
}

const LINE: &str = "0\nLINE\n8\n0\n10\n1.0\n20\n2.0\n11\n4.0\n21\n6.0\n";

fn close(a: Point, b: (f64, f64)) -> bool {
    (a.x - b.0).abs() < 1e-9 && (a.y - b.1).abs() < 1e-9
}

#[test]
fn a_line_lands_in_mm_with_y_turned_down() {
    let imported =
        drawings_from_dxf(&dxf(None, LINE), 0.25, &mut SequentialIds::default()).unwrap();
    assert_eq!(imported.skipped, 0);
    let [
        SchDrawing::Line {
            start, end, width, ..
        },
    ] = imported.drawings.as_slice()
    else {
        panic!("{:?}", imported.drawings);
    };
    assert!(close(*start, (1.0, -2.0)), "{start:?}");
    assert!(close(*end, (4.0, -6.0)), "{end:?}");
    assert_eq!(*width, 0.25);
}

#[test]
fn inches_are_scaled_to_mm() {
    let imported =
        drawings_from_dxf(&dxf(Some(1), LINE), 0.0, &mut SequentialIds::default()).unwrap();
    let SchDrawing::Line { start, .. } = &imported.drawings[0] else {
        panic!();
    };
    assert!(close(*start, (25.4, -50.8)), "{start:?}");
}

#[test]
fn unknown_units_are_refused() {
    assert_eq!(
        drawings_from_dxf(&dxf(Some(3), LINE), 0.0, &mut SequentialIds::default()),
        Err(GraphicsImportError::UnsupportedUnits(3))
    );
}

#[test]
fn a_quarter_arc_keeps_its_middle_on_the_arc() {
    let arc = "0\nARC\n10\n0\n20\n0\n40\n10\n50\n0\n51\n90\n";
    let imported = drawings_from_dxf(&dxf(None, arc), 0.0, &mut SequentialIds::default()).unwrap();
    let SchDrawing::Arc {
        start, mid, end, ..
    } = &imported.drawings[0]
    else {
        panic!("{:?}", imported.drawings);
    };
    assert!(close(*start, (10.0, 0.0)), "{start:?}");
    let h = 10.0 / 2f64.sqrt();
    assert!(close(*mid, (h, -h)), "{mid:?}");
    assert!(close(*end, (0.0, -10.0)), "{end:?}");
}

/// An arc and a circle stored on a plane facing down Z (extrusion 0,0,-1)
/// land mirrored back in X, where the drawing shows them.
#[test]
fn an_arc_on_a_downward_plane_is_mirrored_back_in_x() {
    let arc = "0\nARC\n10\n20\n20\n0\n40\n10\n50\n0\n51\n90\n210\n0\n220\n0\n230\n-1\n";
    let circle = "0\nCIRCLE\n10\n5\n20\n1\n40\n2\n230\n-1.0\n";
    let imported = drawings_from_dxf(
        &dxf(None, &format!("{arc}{circle}")),
        0.0,
        &mut SequentialIds::default(),
    )
    .unwrap();
    let SchDrawing::Arc { start, end, .. } = &imported.drawings[0] else {
        panic!("{:?}", imported.drawings);
    };
    assert!(close(*start, (-30.0, 0.0)), "{start:?}");
    assert!(close(*end, (-20.0, -10.0)), "{end:?}");
    let SchDrawing::Circle { center, .. } = &imported.drawings[1] else {
        panic!("{:?}", imported.drawings);
    };
    assert!(close(*center, (-5.0, -1.0)), "{center:?}");
}

#[test]
fn a_closed_lwpolyline_returns_to_its_first_point() {
    let poly = "0\nLWPOLYLINE\n90\n3\n70\n1\n10\n0\n20\n0\n10\n5\n20\n0\n10\n5\n20\n5\n";
    let imported = drawings_from_dxf(&dxf(None, poly), 0.0, &mut SequentialIds::default()).unwrap();
    let SchDrawing::Polyline { points, .. } = &imported.drawings[0] else {
        panic!();
    };
    assert_eq!(points.len(), 4);
    assert!(close(points[3], (0.0, 0.0)));
}

#[test]
fn a_polyline_takes_its_vertex_rows() {
    let poly = "0\nPOLYLINE\n70\n0\n0\nVERTEX\n10\n0\n20\n0\n0\nVERTEX\n10\n2\n20\n3\n0\nSEQEND\n";
    let imported = drawings_from_dxf(&dxf(None, poly), 0.0, &mut SequentialIds::default()).unwrap();
    let SchDrawing::Polyline { points, .. } = &imported.drawings[0] else {
        panic!("{:?}", imported.drawings);
    };
    assert_eq!(points.len(), 2);
    assert!(close(points[1], (2.0, -3.0)));
}

#[test]
fn other_entities_are_counted_as_skipped() {
    let text = format!("{LINE}0\nTEXT\n10\n0\n20\n0\n1\nhello\n0\nCIRCLE\n10\n1\n20\n1\n40\n0\n");
    let imported =
        drawings_from_dxf(&dxf(None, &text), 0.0, &mut SequentialIds::default()).unwrap();
    assert_eq!(imported.drawings.len(), 1);
    assert_eq!(imported.skipped, 2, "the text and the zero-radius circle");
}

#[test]
fn a_file_with_nothing_drawable_is_refused() {
    assert_eq!(
        drawings_from_dxf(&dxf(None, ""), 0.0, &mut SequentialIds::default()),
        Err(GraphicsImportError::NothingToImport)
    );
}

#[test]
fn a_line_without_its_end_is_refused_by_name() {
    let broken = "0\nLINE\n10\n1\n20\n2\n";
    assert_eq!(
        drawings_from_dxf(&dxf(None, broken), 0.0, &mut SequentialIds::default()),
        Err(GraphicsImportError::Incomplete {
            entity: "LINE".into(),
            what: "second point"
        })
    );
}

#[test]
fn a_code_that_is_not_a_number_is_malformed() {
    assert_eq!(
        drawings_from_dxf(
            "0\nSECTION\nX\nENTITIES\n",
            0.0,
            &mut SequentialIds::default()
        ),
        Err(GraphicsImportError::Malformed { line: 3 })
    );
}
