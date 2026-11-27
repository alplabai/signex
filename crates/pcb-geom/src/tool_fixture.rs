//! A small board for the copper tool tests (#2408): build it up with the
//! helpers, then run a tool on it.

use signex_types::pcb::{
    Footprint, Pad, PadNet, PadShape, PadType, PcbBoard, Point, Segment, Via, ViaType, Zone,
};
use uuid::Uuid;

pub(crate) const MM: i64 = 1_000_000;

/// A 0.2 mm clearance between any two nets.
pub(crate) fn clearance(_: u32, _: u32) -> i64 {
    200_000
}

pub(crate) fn empty_board() -> PcbBoard {
    PcbBoard {
        stackup: None,
        rooms: Vec::new(),
        free: Default::default(),
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
        footprints: Vec::new(),
        segments: Vec::new(),
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        unknown: Default::default(),
    }
}

pub(crate) fn track(n: u128, from: (f64, f64), to: (f64, f64), net: u32) -> Segment {
    Segment {
        uuid: Uuid::from_u128(n),
        start: Point::new(from.0, from.1),
        end: Point::new(to.0, to.1),
        width: 0.25,
        layer: "TopCopper".into(),
        net,
        unknown_columns: Default::default(),
        attributes: Default::default(),
        locked: false,
    }
}

pub(crate) fn via(n: u128, at: (f64, f64), diameter: f64, net: u32) -> Via {
    Via {
        uuid: Uuid::from_u128(n),
        position: Point::new(at.0, at.1),
        diameter,
        drill: diameter / 2.0,
        layers: vec!["TopCopper".into(), "BottomCopper".into()],
        net,
        via_type: ViaType::Through,
        unknown_columns: Default::default(),
        attributes: Default::default(),
        layer_sizes: None,
        locked: false,
    }
}

/// A footprint at `at` with one round SMD pad of `size` on its origin.
pub(crate) fn pad_part(n: u128, at: (f64, f64), size: f64, net: u32) -> Footprint {
    let pad = Pad {
        uuid: Uuid::from_u128(n + 1),
        number: "1".into(),
        pad_type: PadType::Smd,
        shape: PadShape::Circle,
        position: Point::new(0.0, 0.0),
        size: Point::new(size, size),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: (net != 0).then(|| PadNet {
            number: net,
            name: format!("N{net}"),
        }),
        roundrect_ratio: 0.0,
        trapezoid_delta: Default::default(),
        custom_outline: Vec::new(),
        rotation: 0.0,
        net_locked: false,
        thermal: Default::default(),
        layer_sizes: None,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    Footprint {
        uuid: Uuid::from_u128(n),
        reference: format!("U{n}"),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(at.0, at.1),
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

/// A zone over the rectangle `(x0, y0)..(x1, y1)` on the top copper.
pub(crate) fn zone(n: u128, net: u32, fill: &str, rect: (f64, f64, f64, f64)) -> Zone {
    let (x0, y0, x1, y1) = rect;
    Zone {
        uuid: Uuid::from_u128(n),
        net,
        net_name: String::new(),
        layer: "TopCopper".into(),
        outline: vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ],
        holes: Vec::new(),
        priority: 0,
        fill_type: fill.into(),
        thermal_relief: false,
        thermal_gap: 0.0,
        thermal_width: 0.0,
        clearance: 0.0,
        min_thickness: 0.0,
        min_island_area: 0.0,
        remove_dead_islands: false,
        shelved: false,
        unfilled: false,
        corners: Default::default(),
        corner_size: 0.0,
        fill_mode: Default::default(),
        hatch_width: 0.0,
        hatch_gap: 0.0,
        unknown: Default::default(),
    }
}
