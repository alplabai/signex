use serde_json::json;
use signex_types::pcb::PcbBoard;
use signex_types::property::PcbProperty;

use super::*;

fn line(layer: &str, a: (f64, f64), b: (f64, f64)) -> serde_json::Value {
    json!({
        "graphic_type": "line",
        "layer": layer,
        "width": 0.1,
        "start": { "x": a.0, "y": a.1 },
        "end": { "x": b.0, "y": b.1 },
    })
}

fn part(
    reference: &str,
    at: (f64, f64),
    layer: &str,
    graphics: serde_json::Value,
) -> serde_json::Value {
    json!({
        "uuid": uuid::Uuid::now_v7(),
        "reference": reference,
        "value": "10k",
        "footprint_id": "R_0603",
        "position": { "x": at.0, "y": at.1 },
        "rotation": 0.0,
        "layer": layer,
        "graphics": graphics,
    })
}

/// U1 on top with a drawing on each top layer the drawing takes and on
/// three it leaves out, C1 below with a bottom fab body, a top fiducial
/// mark and a dimension on the board, and a 40 x 20 mm outline.
fn board() -> PcbBoard {
    let u1 = part(
        "U1",
        (10.0, 10.0),
        "TopCopper",
        json!([
            line("TopFab", (-1.0, 0.0), (1.0, 0.0)),
            line("TopAssembly", (-1.0, 1.0), (1.0, 1.0)),
            line("TopSilkscreen", (-1.0, 2.0), (1.0, 2.0)),
            line("TopCourtyard", (-1.0, 3.0), (1.0, 3.0)),
            line("BottomFab", (-1.0, 4.0), (1.0, 4.0)),
        ]),
    );
    let c1 = part(
        "C1",
        (30.0, 10.0),
        "BottomCopper",
        json!([line("BottomFab", (-1.0, 0.0), (1.0, 0.0))]),
    );
    serde_json::from_value(json!({
        "uuid": uuid::Uuid::nil(),
        "outline": [
            { "x": 0.0, "y": 0.0 },
            { "x": 40.0, "y": 0.0 },
            { "x": 40.0, "y": 20.0 },
            { "x": 0.0, "y": 20.0 },
        ],
        "footprints": [u1, c1],
        "graphics": [{
            "uuid": uuid::Uuid::now_v7(),
            "graphic_type": "circle",
            "layer": "TopFiducial",
            "width": 0.1,
            "center": { "x": 2.0, "y": 2.0 },
            "radius": 0.5,
        }],
        "free": { "dimensions": [{
            "uuid": uuid::Uuid::now_v7(),
            "start": { "x": 0.0, "y": 20.0 },
            "end": { "x": 40.0, "y": 20.0 },
            "offset": -3.0,
            "layer": "Dimensions",
            "line_width": 0.1,
            "text_height": 1.0,
            "precision": 1,
        }] },
    }))
    .expect("the fixture board")
}

fn inks(drawing: &AssemblyDrawing) -> Vec<AssemblyInk> {
    drawing.marks.iter().map(mark_ink).collect()
}

fn texts(drawing: &AssemblyDrawing, ink: AssemblyInk) -> Vec<String> {
    drawing
        .marks
        .iter()
        .filter_map(|m| match m {
            AssemblyMark::Text { text, ink: i, .. } if *i == ink => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_top_drawing_takes_the_top_assembly_layers_in_drawing_order() {
    let drawing = assembly_drawing(&board(), BoardSide::Top);
    use AssemblyInk as I;
    // 7 dimension strokes and the value, U1's fab and assembly lines, the
    // fiducial, U1's designator, then the outline.
    let mut expected = vec![I::Dimension; 8];
    expected.extend([I::Fab, I::Assembly, I::Fiducial, I::Designator, I::Outline]);
    assert_eq!(inks(&drawing), expected);
    assert_eq!(texts(&drawing, I::Dimension), ["40.0mm"]);
    assert_eq!(texts(&drawing, I::Designator), ["U1"]);
}

#[test]
fn the_bottom_drawing_takes_the_bottom_layers_and_the_bottom_parts() {
    let drawing = assembly_drawing(&board(), BoardSide::Bottom);
    let fab = drawing
        .marks
        .iter()
        .filter(|m| mark_ink(m) == AssemblyInk::Fab)
        .count();
    assert_eq!(fab, 2, "C1's body and U1's bottom fab line");
    assert!(!inks(&drawing).contains(&AssemblyInk::Assembly));
    assert!(!inks(&drawing).contains(&AssemblyInk::Fiducial));
    assert_eq!(texts(&drawing, AssemblyInk::Designator), ["C1"]);
}

#[test]
fn a_designator_sits_on_the_part_origin_unless_it_has_its_own_on_the_layer() {
    let mut board = board();
    let at = |board: &PcbBoard| {
        let drawing = assembly_drawing(board, BoardSide::Top);
        drawing.marks.iter().find_map(|m| match m {
            AssemblyMark::Text {
                at,
                ink: AssemblyInk::Designator,
                ..
            } => Some(*at),
            _ => None,
        })
    };
    assert_eq!(at(&board), Some(Point::new(10.0, 10.0)));
    board.footprints[0].properties.push(PcbProperty {
        key: "Reference".to_string(),
        value: "U1".to_string(),
        position: Some(Point::new(0.0, -2.0)),
        layer: Some("TopDesignator".to_string()),
        ..PcbProperty::default()
    });
    assert_eq!(at(&board), Some(Point::new(10.0, 8.0)));
    board.footprints[0].properties[0].hidden = true;
    assert_eq!(at(&board), None, "a hidden designator is not drawn");
}

#[test]
fn the_extent_is_the_outline_else_the_edge_cuts_else_the_marks() {
    let mut board = board();
    let extent = |board: &PcbBoard| {
        let d = assembly_drawing(board, BoardSide::Top);
        ((d.min.x, d.min.y), (d.max.x, d.max.y))
    };
    assert_eq!(extent(&board), ((0.0, 0.0), (40.0, 20.0)));
    board.outline.clear();
    board.graphics.push(
        serde_json::from_value(json!({
            "graphic_type": "rect",
            "layer": "EdgeCuts",
            "width": 0.05,
            "start": { "x": -5.0, "y": -5.0 },
            "end": { "x": 45.0, "y": 25.0 },
        }))
        .expect("an edge rectangle"),
    );
    assert_eq!(extent(&board), ((-5.0, -5.0), (45.0, 25.0)));
    board.graphics.pop();
    let ((x0, _), (x1, _)) = extent(&board);
    assert!(x0 < 1.5 && x1 > 9.5, "framed on the marks, {x0}..{x1}");
}

#[test]
fn a_dimension_past_the_board_edge_is_inside_the_extent() {
    let mut board = board();
    board.free.dimensions[0].offset = 3.0;
    let drawing = assembly_drawing(&board, BoardSide::Top);
    let dimension_ys: Vec<f64> = drawing
        .marks
        .iter()
        .filter(|m| mark_ink(m) == AssemblyInk::Dimension)
        .flat_map(mark_points)
        .map(|p| p.y)
        .collect();
    let lowest = dimension_ys.iter().copied().fold(f64::MIN, f64::max);
    assert!(lowest >= 23.0, "the fixture puts the line below the board");
    assert!(
        drawing.max.y >= lowest,
        "framed to {}, the dimension reaches {lowest}",
        drawing.max.y
    );
    assert_eq!((drawing.min.x, drawing.min.y), (0.0, 0.0));
    let svg = crate::assembly_svg(&drawing, &crate::AssemblySvgOptions::default());
    let view_box_height: f64 = svg
        .split("viewBox=\"0 0 ")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .and_then(|wh| wh.split_whitespace().nth(1))
        .and_then(|h| h.parse().ok())
        .expect("the SVG has a viewBox");
    // The page starts 2 mm above `min.y`; the dimension must end above its
    // bottom edge.
    assert!(
        view_box_height >= lowest - drawing.min.y + 2.0,
        "viewBox height {view_box_height} cuts the dimension at {lowest}"
    );
}

#[test]
fn a_footprint_slot_on_edge_cuts_is_drawn_as_outline_on_both_sides() {
    let mut board = board();
    board.footprints[0].graphics.push(
        serde_json::from_value(line("EdgeCuts", (-1.0, -3.0), (1.0, -3.0))).expect("a slot edge"),
    );
    for side in [BoardSide::Top, BoardSide::Bottom] {
        let drawing = assembly_drawing(&board, side);
        let slot = drawing.marks.iter().any(|m| {
            matches!(m, AssemblyMark::Stroke { points, ink: AssemblyInk::Outline, .. }
                if points == &[Point::new(9.0, 7.0), Point::new(11.0, 7.0)])
        });
        assert!(slot, "{side:?}: U1's slot at y=7 is drawn");
    }
}

#[test]
fn only_the_sides_documentation_layers_and_the_shared_ones_have_an_ink() {
    use signex_types::pcb_layer::CopperLayer;
    let t = |layer| PcbLayer::Technical(layer);
    let top = BoardSide::Top;
    assert_eq!(
        assembly_ink(top, t(TechnicalLayer::FabTop)),
        Some(AssemblyInk::Fab)
    );
    assert_eq!(assembly_ink(top, t(TechnicalLayer::FabBottom)), None);
    assert_eq!(
        assembly_ink(BoardSide::Bottom, t(TechnicalLayer::Dimensions)),
        Some(AssemblyInk::Dimension)
    );
    assert_eq!(
        assembly_ink(BoardSide::Bottom, t(TechnicalLayer::EdgeCuts)),
        Some(AssemblyInk::Outline)
    );
    assert_eq!(assembly_ink(top, t(TechnicalLayer::SilkscreenTop)), None);
    assert_eq!(assembly_ink(top, PcbLayer::Copper(CopperLayer::Top)), None);
}

/// The realistic corpus board with parts and fab bodies on both sides and
/// an outline of Edge Cuts lines and an arc.
pub(crate) fn real_board() -> PcbBoard {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../signex-types/tests/corpus/pcb/real/mixed-tht-smd.snxpcb");
    let text = std::fs::read_to_string(path).expect("corpus file");
    signex_types::format::SnxPcb::parse(&text)
        .expect("board parses")
        .board
}

/// Compare `actual` with the golden file `testdata/assembly/<name>`;
/// `UPDATE_GOLDEN=1` writes it instead. A diff is a change to the drawing,
/// not a snapshot to refresh without a look.
pub(crate) fn check_golden(name: &str, actual: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata/assembly")
        .join(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("golden folder");
        std::fs::write(&path, actual).expect("golden written");
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}; run with UPDATE_GOLDEN=1", path.display()))
        .replace("\r\n", "\n");
    assert!(
        expected == actual,
        "{} differs from the drawing; regenerate with UPDATE_GOLDEN=1 \
         cargo test -p signex-output assembly if the change is meant",
        path.display()
    );
}

#[test]
fn the_real_board_draws_both_sides_framed_on_its_edge_cuts() {
    let board = real_board();
    let top = assembly_drawing(&board, BoardSide::Top);
    let bottom = assembly_drawing(&board, BoardSide::Bottom);
    assert_eq!((top.min, top.max), (bottom.min, bottom.max));
    assert_eq!(top.min, Point::new(0.0, 0.0));
    let designators = |d: &AssemblyDrawing| texts(d, AssemblyInk::Designator);
    assert_eq!(designators(&bottom), ["C3", "C4"]);
    assert_eq!(designators(&top).len(), 8);
}
