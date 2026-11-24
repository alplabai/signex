//! #2868: a board dimension's look on the canvas — arrow style, extension
//! width, text position and the value's face.

use signex_types::pcb::Point;
use signex_types::pcb_dimension_style::{ArrowStyle, BoardFont, DimensionTextPosition};
use signex_types::pcb_layer::TechnicalLayer;
use uuid::Uuid;

use super::*;
use crate::pcb::PcbSnapshot;

const DIMENSIONS: PcbLayer = PcbLayer::Technical(TechnicalLayer::Dimensions);

/// A board holding `dimension` alone.
fn drawn(dimension: LinearDimension) -> PcbSnapshot {
    let mut board: PcbBoard =
        serde_json::from_value(serde_json::json!({ "uuid": Uuid::from_u128(1) })).unwrap();
    board.free.dimensions.push(dimension);
    PcbSnapshot::from_board_with_view(&board, &PcbLayerView::default())
}

/// A 10 mm dimension along y = 0, standing 2 mm above it.
fn dimension() -> LinearDimension {
    LinearDimension::new(
        Uuid::from_u128(2),
        Point::new(0.0, 0.0),
        Point::new(10.0, 0.0),
        2.0,
    )
}

fn lines(snapshot: &PcbSnapshot) -> Vec<&GraphicInput> {
    let line = |g: &&GraphicInput| matches!(g.shape, GraphicShapeInput::Line { .. });
    snapshot
        .graphics
        .iter()
        .filter(|g| g.layer == DIMENSIONS)
        .filter(line)
        .collect()
}

fn filled(snapshot: &PcbSnapshot) -> Vec<&GraphicInput> {
    snapshot
        .graphics
        .iter()
        .filter(|g| g.layer == DIMENSIONS && g.filled)
        .collect()
}

#[test]
fn open_arrows_are_strokes_and_filled_arrows_are_filled_triangles() {
    let open = drawn(dimension());
    assert_eq!(
        lines(&open).len(),
        7,
        "two extensions, the line, four wings"
    );
    assert!(filled(&open).is_empty());

    let mut d = dimension();
    d.style.arrow_style = ArrowStyle::Filled;
    let solid = drawn(d);
    assert_eq!(lines(&solid).len(), 3, "two extensions and the line");
    let heads = filled(&solid);
    assert_eq!(heads.len(), 2);
    for head in heads {
        let GraphicShapeInput::Polygon { points } = &head.shape else {
            panic!("an arrowhead is a polygon: {head:?}");
        };
        assert_eq!(points.len(), 3);
    }

    let mut d = dimension();
    d.style.arrow_style = ArrowStyle::None;
    let bare = drawn(d);
    assert_eq!(lines(&bare).len(), 3);
    assert!(filled(&bare).is_empty());
}

#[test]
fn extension_lines_take_the_extension_width() {
    let mut d = dimension();
    d.style.extension_width = 0.3;
    let snapshot = drawn(d);
    let widths: Vec<f32> = lines(&snapshot).iter().map(|g| g.width_mm).collect();
    assert_eq!(&widths[..2], &[0.3, 0.3]);
    assert!(
        widths[2..].iter().all(|w| (*w - 0.1).abs() < 1e-6),
        "{widths:?}"
    );
}

#[test]
fn the_value_sits_above_the_line_or_on_it() {
    let above = drawn(dimension());
    assert_eq!(above.texts[0].position, [5.0, 3.0]);

    let mut d = dimension();
    d.style.text_position = DimensionTextPosition::Inline;
    let inline = drawn(d);
    assert_eq!(inline.texts[0].position, [5.0, 2.0]);
    // The line breaks around the value: two pieces.
    assert_eq!(lines(&inline).len(), 8);
}

#[test]
fn the_value_is_drawn_in_its_face() {
    assert_eq!(drawn(dimension()).texts[0].face, TextFace::PLAIN);
    let mut d = dimension();
    d.style.font = BoardFont::Sans;
    d.style.bold = true;
    d.style.italic = true;
    d.style.text_width = 1.2;
    let face = drawn(d).texts[0].face;
    assert_eq!(face.font, BoardFont::Sans);
    assert!(face.bold && face.italic);
    assert!((face.stretch - 2.0).abs() < 1e-6, "{face:?}");
}
