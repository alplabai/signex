use signex_types::pcb::{Footprint, FpGraphic, Pad, PadShape, PadType, PcbBoard, Point};
use uuid::Uuid;

use super::*;
use crate::{BoardIndex, ItemRef, Selection, pick};

fn pad(x: f64, y: f64, w: f64, h: f64, rotation: f64) -> Pad {
    Pad {
        uuid: Uuid::now_v7(),
        number: "1".into(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: Point::new(x, y),
        size: Point::new(w, h),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: None,
        roundrect_ratio: 0.0,
        rotation,
        net_locked: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn footprint(pads: Vec<Pad>, graphics: Vec<FpGraphic>, rotation: f64, layer: &str) -> Footprint {
    Footprint {
        uuid: Uuid::from_u128(7),
        reference: "U1".into(),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(10.0, 20.0),
        rotation,
        layer: layer.into(),
        locked: false,
        schematic_link: None,
        pads,
        graphics,
        properties: Vec::new(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn courtyard_rect(from: (f64, f64), to: (f64, f64)) -> FpGraphic {
    FpGraphic {
        graphic_type: "rect".into(),
        layer: "TopCourtyard".into(),
        width: 0.0,
        start: Some(Point::new(from.0, from.1)),
        end: Some(Point::new(to.0, to.1)),
        center: None,
        mid: None,
        radius: 0.0,
        points: Vec::new(),
        text: String::new(),
        font_size: 0.0,
        position: None,
        rotation: 0.0,
        fill: String::new(),
    }
}

fn nm(mm: f64) -> i64 {
    (mm * 1_000_000.0).round() as i64
}

#[test]
fn the_pads_give_the_body_when_there_is_no_courtyard() {
    let fp = footprint(
        vec![pad(-2.0, 0.0, 1.0, 1.0, 0.0), pad(2.0, 0.0, 1.0, 1.0, 90.0)],
        Vec::new(),
        0.0,
        "TopCopper",
    );
    let body = footprint_body(&fp).unwrap();
    assert_eq!(
        body,
        Shape::Rect {
            center: PointNm::new(nm(10.0), nm(20.0)),
            half_w: nm(2.5),
            half_h: nm(0.5),
            rotation_deg: 0.0,
        }
    );
}

#[test]
fn the_courtyard_wins_over_the_pads_and_turns_with_the_footprint() {
    let fp = footprint(
        vec![pad(0.0, 0.0, 1.0, 1.0, 0.0)],
        vec![courtyard_rect((1.0, -1.0), (5.0, 1.0))],
        90.0,
        "TopCopper",
    );
    let Shape::Rect {
        center,
        half_w,
        half_h,
        rotation_deg,
    } = footprint_body(&fp).unwrap()
    else {
        panic!("a body is a rectangle");
    };
    // Local centre (3, 0) turned 90° counter-clockwise lands at (0, 3).
    assert_eq!(center, PointNm::new(nm(10.0), nm(23.0)));
    assert_eq!((half_w, half_h, rotation_deg), (nm(2.0), nm(1.0), 90.0));
}

#[test]
fn a_bottom_footprint_mirrors_its_body() {
    let fp = footprint(
        Vec::new(),
        vec![courtyard_rect((1.0, -1.0), (5.0, 1.0))],
        0.0,
        "BottomCopper",
    );
    let Shape::Rect { center, .. } = footprint_body(&fp).unwrap() else {
        panic!("a body is a rectangle");
    };
    assert_eq!(center, PointNm::new(nm(7.0), nm(20.0)));
}

#[test]
fn an_empty_footprint_is_a_point_at_its_origin() {
    let fp = footprint(Vec::new(), Vec::new(), 0.0, "TopCopper");
    assert_eq!(
        footprint_body(&fp).unwrap(),
        Shape::Disc {
            center: PointNm::new(nm(10.0), nm(20.0)),
            radius: 0,
        }
    );
}

#[test]
fn a_non_finite_rotation_is_refused() {
    let fp = footprint(Vec::new(), Vec::new(), f64::NAN, "TopCopper");
    assert!(footprint_body(&fp).is_err());
}

fn board(fp: Footprint) -> PcbBoard {
    PcbBoard {
        uuid: Uuid::nil(),
        version: 1,
        generator: String::new(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: Vec::new(),
        setup: None,
        nets: Vec::new(),
        footprints: vec![fp],
        segments: Vec::new(),
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        unknown: Default::default(),
    }
}

#[test]
fn a_click_on_a_pad_takes_the_pad_and_between_pads_the_footprint() {
    let fp = footprint(
        vec![pad(-2.0, 0.0, 1.0, 1.0, 0.0), pad(2.0, 0.0, 1.0, 1.0, 0.0)],
        Vec::new(),
        0.0,
        "TopCopper",
    );
    let pad_id = fp.pads[0].uuid;
    let index = BoardIndex::build_selectable(&board(fp));
    let at = |x: f64| PointNm::new(nm(x), nm(20.0));
    let on_pad = pick(&index, at(8.0), 0).map(|i| i.item);
    assert_eq!(
        on_pad,
        Some(ItemRef::Pad {
            footprint: Uuid::from_u128(7),
            pad: pad_id
        })
    );
    let between = pick(&index, at(10.0), 0).map(|i| i.item);
    assert_eq!(between, Some(ItemRef::Footprint(Uuid::from_u128(7))));
}

#[test]
fn the_copper_index_holds_no_bodies() {
    let fp = footprint(
        vec![pad(0.0, 0.0, 1.0, 1.0, 0.0)],
        Vec::new(),
        0.0,
        "TopCopper",
    );
    let b = board(fp);
    assert_eq!(BoardIndex::build(&b).len(), 1);
    assert_eq!(BoardIndex::build_selectable(&b).len(), 2);
}

#[test]
fn a_selected_pad_stands_for_its_footprint() {
    let mut selection = Selection::default();
    let footprint = Uuid::from_u128(7);
    selection.apply_click(
        Some(ItemRef::Pad {
            footprint,
            pad: Uuid::from_u128(8),
        }),
        crate::ClickMode::Replace,
    );
    selection.apply_click(
        Some(ItemRef::Track(Uuid::from_u128(9))),
        crate::ClickMode::Add,
    );
    assert_eq!(
        selection.footprints().into_iter().collect::<Vec<_>>(),
        vec![footprint]
    );
}
