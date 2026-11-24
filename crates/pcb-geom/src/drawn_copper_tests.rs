//! #2883: board drawings and strings on copper are copper of no net.

use signex_types::pcb::{BoardGraphic, BoardText, PcbBoard, Point};
use uuid::Uuid;

use super::drawn_copper;
use crate::{ItemRef, PickKind, PointNm, RectNm, Shape, collect_copper};

fn board() -> PcbBoard {
    PcbBoard {
        sketch: None,
        stackup: None,
        rooms: Vec::new(),
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
        graphics: Vec::new(),
        texts: Vec::new(),
        free: Default::default(),
        options: Default::default(),
        unknown: Default::default(),
    }
}

fn circle(layer: &str) -> BoardGraphic {
    BoardGraphic {
        uuid: Uuid::from_u128(1),
        graphic_type: "circle".into(),
        layer: layer.into(),
        width: 0.2,
        start: None,
        end: Some(Point::new(15.0, 10.0)),
        center: Some(Point::new(10.0, 10.0)),
        radius: 5.0,
        points: Vec::new(),
        text: String::new(),
        font_size: 0.0,
    }
}

fn string(layer: &str) -> BoardText {
    BoardText {
        uuid: Uuid::from_u128(2),
        text: "A1".into(),
        position: Point::new(30.0, 30.0),
        layer: layer.into(),
        font_size: 1.0,
        rotation: 0.0,
    }
}

fn at(x: f64, y: f64) -> PointNm {
    PointNm::from_mm(Point::new(x, y), "at").unwrap()
}

#[test]
fn a_copper_circle_is_copper_of_no_net_along_its_stroke() {
    let mut b = board();
    b.graphics.push(circle("TopCopper"));
    let copper = drawn_copper(&b);
    assert!(copper.len() > 36, "finer chords than the pick index");
    let id = ItemRef::Graphic {
        graphic: Uuid::from_u128(1),
        kind: PickKind::Other,
    };
    assert!(copper.iter().all(|c| c.item == id && c.net == 0));
    assert!(copper.iter().any(|c| c.shape.contains(at(15.0, 10.0), 0)));
    assert!(
        !copper.iter().any(|c| c.shape.contains(at(10.0, 10.0), 0)),
        "an outline, not a disc"
    );
    // Every chord stays within 5 µm of the circle.
    for c in &copper {
        let Shape::Capsule { a, .. } = c.shape else {
            panic!("a stroke piece");
        };
        let r = ((a.x - 10_000_000) as f64).hypot((a.y - 10_000_000) as f64);
        assert!((r - 5_000_000.0).abs() <= 1.0, "{r}");
    }
    assert!(
        collect_copper(&b).is_empty(),
        "the netted list is unchanged"
    );
}

#[test]
fn a_copper_string_is_copper_along_its_glyphs() {
    let mut b = board();
    b.texts.push(string("BottomCopper"));
    let copper = drawn_copper(&b);
    assert!(!copper.is_empty());
    assert!(
        copper
            .iter()
            .all(|c| c.item == ItemRef::Text(Uuid::from_u128(2)) && c.net == 0)
    );
    let about = RectNm::new(at(28.0, 28.0), at(32.0, 32.0));
    assert!(
        copper.iter().all(|c| c.shape.aabb().intersects(&about)),
        "the glyphs sit about the string's position"
    );
}

#[test]
fn drawings_and_strings_off_copper_are_not_copper() {
    let mut b = board();
    b.graphics.push(circle("TopSilkscreen"));
    b.texts.push(string("TopSilkscreen"));
    assert!(drawn_copper(&b).is_empty());
}
