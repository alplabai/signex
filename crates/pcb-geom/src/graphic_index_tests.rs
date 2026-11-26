use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use super::*;
use crate::index::{BoardIndex, RectSelect};

const MM: i64 = 1_000_000;

fn drawing(n: u128, kind: &str, a: (f64, f64), b: (f64, f64)) -> BoardGraphic {
    BoardGraphic {
        uuid: Uuid::from_u128(n),
        graphic_type: kind.into(),
        layer: "TopSilkscreen".into(),
        width: 0.2,
        start: Some(Point::new(a.0, a.1)),
        end: Some(Point::new(b.0, b.1)),
        center: Some(Point::new(a.0, a.1)),
        radius: 0.0,
        points: Vec::new(),
        text: "NOTE".into(),
        font_size: 1.0,
    }
}

fn board(graphics: Vec<BoardGraphic>) -> PcbBoard {
    PcbBoard {
        sketch: None,
        stackup: None,
        rooms: Vec::new(),
        free: Default::default(),
        options: Default::default(),
        uuid: Uuid::nil(),
        version: 1,
        generator: String::new(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: Vec::new(),
        setup: None,
        nets: Vec::new(),
        footprints: Vec::new(),
        segments: Vec::new(),
        vias: Vec::new(),
        zones: Vec::new(),
        graphics,
        texts: Vec::new(),
        unknown: Default::default(),
    }
}

fn line_ref(n: u128) -> ItemRef {
    ItemRef::Graphic {
        graphic: Uuid::from_u128(n),
        kind: PickKind::Other,
    }
}

/// #2424: a line is picked on its stroke, a box takes it whole only when it
/// holds the whole line, and connectivity holds no drawings.
#[test]
fn a_line_is_picked_on_its_stroke() {
    let b = board(vec![drawing(1, "line", (0.0, 0.0), (10.0, 0.0))]);
    let index = BoardIndex::build_selectable(&b);
    assert_eq!(
        index.hit_point(PointNm::new(5 * MM, 0), 0),
        vec![line_ref(1)]
    );
    assert_eq!(
        index.hit_point(PointNm::new(5 * MM, MM / 20), 0),
        vec![line_ref(1)],
        "within its half width"
    );
    assert!(index.hit_point(PointNm::new(5 * MM, MM), 0).is_empty());
    let whole = RectNm::new(PointNm::new(-MM, -MM), PointNm::new(11 * MM, MM));
    let half = RectNm::new(PointNm::new(-MM, -MM), PointNm::new(5 * MM, MM));
    assert_eq!(
        index.hit_rect(&whole, RectSelect::Inside),
        vec![line_ref(1)]
    );
    assert_eq!(
        index.hit_rect(&half, RectSelect::Touching),
        vec![line_ref(1)]
    );
    assert!(BoardIndex::build(&b).is_empty());
}

/// A rectangle is an outline: inside it, off its stroke, nothing is picked.
/// A text frame's box is picked inside too, as a text.
#[test]
fn a_rectangle_is_picked_on_its_outline_and_a_text_frame_inside() {
    let b = board(vec![
        drawing(2, "rect", (0.0, 0.0), (10.0, 10.0)),
        drawing(3, "text_frame", (20.0, 0.0), (30.0, 10.0)),
    ]);
    let index = BoardIndex::build_selectable(&b);
    assert_eq!(
        index.hit_point(PointNm::new(0, 5 * MM), 0),
        vec![line_ref(2)]
    );
    assert!(index.hit_point(PointNm::new(5 * MM, 5 * MM), 0).is_empty());
    let frame = ItemRef::Graphic {
        graphic: Uuid::from_u128(3),
        kind: PickKind::Texts,
    };
    assert_eq!(
        index.hit_point(PointNm::new(25 * MM, 5 * MM), 0),
        vec![frame]
    );
    let inner = RectNm::new(PointNm::new(24 * MM, 4 * MM), PointNm::new(26 * MM, 6 * MM));
    assert_eq!(index.hit_rect(&inner, RectSelect::Touching), vec![frame]);
}

/// A circle is picked on its rim, not at its centre.
#[test]
fn a_circle_is_picked_on_its_rim() {
    let mut circle = drawing(4, "circle", (0.0, 0.0), (5.0, 0.0));
    circle.radius = 5.0;
    let index = BoardIndex::build_selectable(&board(vec![circle]));
    assert_eq!(
        index.hit_point(PointNm::new(0, 5 * MM), MM / 10),
        vec![line_ref(4)]
    );
    assert!(index.hit_point(PointNm::new(0, 0), 0).is_empty());
}

#[test]
fn two_drawings_with_one_id_are_refused() {
    let b = board(vec![
        drawing(5, "line", (0.0, 0.0), (1.0, 0.0)),
        drawing(5, "line", (0.0, 2.0), (1.0, 2.0)),
    ]);
    let index = BoardIndex::build_selectable(&b);
    assert_eq!(index.rejected().len(), 1);
    assert_eq!(index.rejected()[0].error, GeomError::DuplicateItem);
}
