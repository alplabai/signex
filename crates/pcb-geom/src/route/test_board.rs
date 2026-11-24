//! Boards for the router's tests: round top-side pads and 0.25 mm top
//! tracks, built from plain values.

use signex_types::pcb::{Footprint, Pad, PadNet, PadShape, PadType, PcbBoard, Point, Segment};
use uuid::Uuid;

use crate::units::PointNm;

/// `mm` in nm, rounded.
pub(crate) fn nm(mm: f64) -> i64 {
    (mm * 1.0e6).round() as i64
}

pub(crate) fn at(x: f64, y: f64) -> PointNm {
    PointNm::new(nm(x), nm(y))
}

/// A footprint `n` at (`x`, `y`) holding one round top pad of diameter
/// `size` on `net`.
pub(crate) fn pad_at(n: u128, x: f64, y: f64, size: f64, net: u32) -> Footprint {
    let pad = Pad {
        uuid: Uuid::from_u128(5000 + n),
        number: "1".into(),
        pad_type: PadType::Smd,
        shape: PadShape::Circle,
        position: Point::new(0.0, 0.0),
        size: Point::new(size, size),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: Some(PadNet {
            number: net,
            name: format!("N{net}"),
        }),
        roundrect_ratio: 0.0,
        trapezoid_delta: Default::default(),
        custom_outline: Vec::new(),
        rotation: 0.0,
        net_locked: false,
        layer_sizes: None,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    Footprint {
        uuid: Uuid::from_u128(4000 + n),
        reference: format!("U{n}"),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(x, y),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        schematic_link: None,
        library_ref: None,
        component: Default::default(),
        pads: vec![pad],
        graphics: Vec::new(),
        properties: Vec::new(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

/// A 0.25 mm top track with id `n`.
pub(crate) fn track_at(n: u128, a: (f64, f64), b: (f64, f64), net: u32) -> Segment {
    Segment {
        uuid: Uuid::from_u128(n),
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        width: 0.25,
        layer: "TopCopper".into(),
        net,
        unknown_columns: Default::default(),
        attributes: Default::default(),
        locked: false,
    }
}

pub(crate) fn board(footprints: Vec<Footprint>, segments: Vec<Segment>) -> PcbBoard {
    PcbBoard {
        stackup: None,
        rooms: Vec::new(),
        sketch: None,
        options: Default::default(),
        uuid: Uuid::nil(),
        version: 1,
        generator: String::new(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: Vec::new(),
        setup: None,
        nets: Vec::new(),
        footprints,
        segments,
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        free: Default::default(),
        unknown: Default::default(),
    }
}
