//! The fixture board of the fabrication tests: a 20 x 10 mm two-layer
//! board with a two-pad SMD resistor, a through-hole pin, one track and
//! one via.

use signex_types::pcb::{
    DrillDef, Footprint, LayerDef, NetDef, Pad, PadNet, PadShape, PadType, PcbBoard, Point,
    Segment, Via, ViaType,
};
use uuid::Uuid;

pub(crate) fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

pub(crate) fn empty_board() -> PcbBoard {
    PcbBoard {
        stackup: None,
        uuid: Uuid::from_u128(1),
        version: 1,
        generator: String::new(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: vec![layer(0, "TopCopper"), layer(31, "BottomCopper")],
        setup: None,
        nets: Vec::new(),
        footprints: Vec::new(),
        segments: Vec::new(),
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        free: Default::default(),
        sketch: None,
        options: Default::default(),
        unknown: Default::default(),
    }
}

fn layer(id: u8, name: &str) -> LayerDef {
    LayerDef {
        id,
        name: name.into(),
        layer_type: "signal".into(),
        display_name: String::new(),
    }
}

fn net(number: u32, name: &str) -> NetDef {
    NetDef {
        number,
        name: name.into(),
    }
}

pub(crate) fn footprint(
    id: u128,
    reference: &str,
    at: Point,
    rotation: f64,
    layer: &str,
    pads: Vec<Pad>,
) -> Footprint {
    Footprint {
        uuid: Uuid::from_u128(id),
        reference: reference.into(),
        value: String::new(),
        footprint_id: String::new(),
        position: at,
        rotation,
        layer: layer.into(),
        locked: false,
        pads,
        graphics: Vec::new(),
        properties: Vec::new(),
        schematic_link: None,
        library_ref: None,
        component: Default::default(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

pub(crate) fn pad(
    number: &str,
    pad_type: PadType,
    shape: PadShape,
    at: Point,
    size: Point,
    layers: &[&str],
) -> Pad {
    Pad {
        uuid: Uuid::from_u128(1000 + number.len() as u128),
        number: number.into(),
        pad_type,
        shape,
        position: at,
        size,
        drill: None,
        layers: layers.iter().map(|l| l.to_string()).collect(),
        net: None,
        roundrect_ratio: 0.0,
        trapezoid_delta: Default::default(),
        custom_outline: Vec::new(),
        rotation: 0.0,
        net_locked: false,
        layer_sizes: None,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

pub(crate) fn drilled(pad: Pad, diameter: f64, shape: &str) -> Pad {
    Pad {
        drill: Some(DrillDef {
            diameter,
            shape: shape.into(),
        }),
        ..pad
    }
}

fn netted(pad: Pad, number: u32, name: &str) -> Pad {
    Pad {
        net: Some(PadNet {
            number,
            name: name.into(),
        }),
        ..pad
    }
}

const SMD_TOP: [&str; 3] = ["TopCopper", "TopSolderMask", "TopPaste"];
const THRU: [&str; 4] = [
    "TopCopper",
    "BottomCopper",
    "TopSolderMask",
    "BottomSolderMask",
];

/// The golden-file board.
pub(crate) fn fixture_board() -> PcbBoard {
    let mut board = empty_board();
    board.outline = vec![p(0.0, 0.0), p(20.0, 0.0), p(20.0, 10.0), p(0.0, 10.0)];
    board.nets = vec![net(1, "GND"), net(2, "VCC")];
    let smd = |number: &str, x: f64| {
        pad(
            number,
            PadType::Smd,
            PadShape::Rect,
            p(x, 0.0),
            p(1.0, 1.2),
            &SMD_TOP,
        )
    };
    let mut r1 = footprint(
        10,
        "R1",
        p(5.0, 5.0),
        0.0,
        "TopCopper",
        vec![
            netted(smd("1", -1.0), 1, "GND"),
            netted(smd("2", 1.0), 2, "VCC"),
        ],
    );
    r1.value = "10k".into();
    r1.footprint_id = "R_0603".into();
    let pin = pad(
        "1",
        PadType::Thru,
        PadShape::Circle,
        p(0.0, 0.0),
        p(1.6, 1.6),
        &THRU,
    );
    let mut j1 = footprint(
        20,
        "J1",
        p(10.0, 8.0),
        0.0,
        "TopCopper",
        vec![netted(drilled(pin, 0.8, ""), 1, "GND")],
    );
    j1.value = "CONN".into();
    j1.footprint_id = "PIN_1".into();
    board.footprints = vec![r1, j1];
    board.segments = vec![Segment {
        uuid: Uuid::from_u128(30),
        start: p(6.0, 5.0),
        end: p(15.0, 5.0),
        width: 0.25,
        layer: "TopCopper".into(),
        net: 2,
        unknown_columns: Default::default(),
        attributes: Default::default(),
        locked: false,
    }];
    board.vias = vec![Via {
        uuid: Uuid::from_u128(40),
        position: p(15.0, 5.0),
        diameter: 0.6,
        drill: 0.3,
        layers: vec!["TopCopper".into(), "BottomCopper".into()],
        net: 2,
        via_type: ViaType::Through,
        unknown_columns: Default::default(),
        attributes: Default::default(),
        layer_sizes: None,
        locked: false,
    }];
    board
}
