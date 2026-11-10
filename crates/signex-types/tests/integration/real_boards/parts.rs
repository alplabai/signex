//! Footprints for the realistic corpus boards, in footprint-local mm.
//!
//! Land patterns follow the usual IPC-7351 nominal sizes closely enough that
//! a reviewer reads them as the parts they are named for. Each part carries
//! silk, a fab outline and a courtyard; pad uuids and nets are filled in when
//! the part is placed.

use signex_types::pcb::{DrillDef, FpGraphic, Pad, PadShape, PadType, Point};
use uuid::Uuid;

use super::builder::p;

/// A footprint before placement.
pub struct Part {
    pub id: &'static str,
    pub pads: Vec<Pad>,
    pub graphics: Vec<FpGraphic>,
    /// Where the reference designator sits, footprint-local.
    pub label_at: Point,
}

const SMD_LAYERS: [&str; 3] = ["TopCopper", "TopSolderMask", "TopPaste"];
const THT_LAYERS: [&str; 4] = [
    "TopCopper",
    "BottomCopper",
    "TopSolderMask",
    "BottomSolderMask",
];

fn pad(number: &str, kind: PadType, shape: PadShape, at: Point, size: Point) -> Pad {
    let layers: &[&str] = if kind == PadType::Smd {
        &SMD_LAYERS
    } else {
        &THT_LAYERS
    };
    Pad {
        uuid: Uuid::nil(),
        number: number.to_string(),
        pad_type: kind,
        shape,
        position: at,
        size,
        drill: None,
        layers: layers.iter().map(|l| l.to_string()).collect(),
        net: None,
        roundrect_ratio: 0.0,
        rotation: 0.0,
        net_locked: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

/// A rounded-rectangle surface pad, corner ratio 0.25.
fn smd(number: &str, x: f64, y: f64, w: f64, h: f64) -> Pad {
    Pad {
        roundrect_ratio: 0.25,
        ..pad(number, PadType::Smd, PadShape::RoundRect, p(x, y), p(w, h))
    }
}

/// A plated through-hole pad with a round drill.
fn tht(number: &str, shape: PadShape, x: f64, y: f64, size: f64, drill: f64) -> Pad {
    Pad {
        drill: Some(DrillDef {
            diameter: drill,
            shape: String::new(),
        }),
        ..pad(number, PadType::Thru, shape, p(x, y), p(size, size))
    }
}

/// A plated slot: an oval pad with an oval drill of width `drill`.
fn slot(number: &str, x: f64, y: f64, w: f64, h: f64, drill: f64) -> Pad {
    Pad {
        drill: Some(DrillDef {
            diameter: drill,
            shape: "oval".into(),
        }),
        ..pad(number, PadType::Thru, PadShape::Oval, p(x, y), p(w, h))
    }
}

fn graphic(kind: &str, layer: &str, width: f64) -> FpGraphic {
    FpGraphic {
        graphic_type: kind.into(),
        layer: layer.into(),
        width,
        start: None,
        end: None,
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

fn line(layer: &str, a: Point, b: Point) -> FpGraphic {
    FpGraphic {
        start: Some(a),
        end: Some(b),
        ..graphic("line", layer, 0.12)
    }
}

fn circle(layer: &str, c: Point, r: f64, width: f64) -> FpGraphic {
    FpGraphic {
        center: Some(c),
        radius: r,
        ..graphic("circle", layer, width)
    }
}

/// The fab body outline and the courtyard `margin` beyond `(hw, hh)`.
fn body(hw: f64, hh: f64, court_hw: f64, court_hh: f64) -> Vec<FpGraphic> {
    vec![
        FpGraphic {
            start: Some(p(-hw, -hh)),
            end: Some(p(hw, hh)),
            ..graphic("rect", "TopFab", 0.1)
        },
        FpGraphic {
            points: vec![
                p(-court_hw, -court_hh),
                p(court_hw, -court_hh),
                p(court_hw, court_hh),
                p(-court_hw, court_hh),
            ],
            ..graphic("poly", "TopCourtyard", 0.05)
        },
    ]
}

/// A two-terminal chip part: pads at `x = +-c`, each `w` x `h`; body
/// `(bw, bh)`.
fn two_terminal(id: &'static str, c: f64, w: f64, h: f64, bw: f64, bh: f64) -> Part {
    let court_hw = c + w / 2.0 + 0.25;
    let court_hh = (h / 2.0).max(bh / 2.0) + 0.25;
    let mut graphics = body(bw / 2.0, bh / 2.0, court_hw, court_hh);
    // Silk only along the long sides, clear of the pads.
    let sx = (c - w / 2.0 - 0.2).max(0.1);
    let sy = court_hh - 0.1;
    graphics.push(line("TopSilkscreen", p(-sx, -sy), p(sx, -sy)));
    graphics.push(line("TopSilkscreen", p(-sx, sy), p(sx, sy)));
    Part {
        id,
        pads: vec![smd("1", -c, 0.0, w, h), smd("2", c, 0.0, w, h)],
        graphics,
        label_at: p(0.0, -court_hh - 0.8),
    }
}

pub fn r0603() -> Part {
    two_terminal("R_0603", 0.825, 0.8, 0.95, 1.6, 0.8)
}

pub fn c0603() -> Part {
    two_terminal("C_0603", 0.775, 0.9, 0.95, 1.6, 0.8)
}

pub fn c0805() -> Part {
    two_terminal("C_0805", 0.95, 1.0, 1.45, 2.0, 1.25)
}

pub fn r0805() -> Part {
    two_terminal("R_0805", 0.9125, 1.025, 1.4, 2.0, 1.25)
}

/// Pin 1 anode, pin 2 cathode (as the schematic LED symbol numbers them);
/// the dot marks the cathode.
pub fn led0805() -> Part {
    led(two_terminal("LED_0805", 0.9375, 0.975, 1.4, 2.0, 1.25), 2.0)
}

/// As [`led0805`], in 0603.
pub fn led0603() -> Part {
    led(
        two_terminal("LED_0603", 0.7875, 0.875, 0.95, 1.6, 0.8),
        1.75,
    )
}

fn led(mut part: Part, dot_x: f64) -> Part {
    part.graphics
        .push(circle("TopSilkscreen", p(dot_x, 0.0), 0.1, 0.2));
    part
}

pub fn fuse1206() -> Part {
    two_terminal("Fuse_1206", 1.5, 1.15, 1.8, 3.2, 1.6)
}

/// Pin 1 cathode (band), pin 2 anode.
pub fn sod123() -> Part {
    let mut part = two_terminal("SOD-123", 1.635, 0.91, 1.22, 2.7, 1.6);
    part.graphics
        .push(line("TopSilkscreen", p(-2.35, -0.9), p(-2.35, 0.9)));
    part
}

/// SOT-223: pins 1-3 on one side at 2.3 mm pitch, the tab (pin 4) opposite.
pub fn sot223() -> Part {
    let mut graphics = body(1.75, 3.25, 4.05, 4.35);
    graphics.push(line("TopSilkscreen", p(-1.91, -3.41), p(1.91, -3.41)));
    graphics.push(line("TopSilkscreen", p(-1.91, 3.41), p(-3.15, 3.41)));
    Part {
        id: "SOT-223",
        pads: vec![
            smd("1", -2.3, 3.15, 1.0, 1.8),
            smd("2", 0.0, 3.15, 1.0, 1.8),
            smd("3", 2.3, 3.15, 1.0, 1.8),
            smd("4", 0.0, -3.15, 3.3, 1.8),
        ],
        graphics,
        label_at: p(0.0, -5.0),
    }
}

/// SOT-23-5: pins 1-3 along +y, pins 4-5 along -y.
pub fn sot23_5() -> Part {
    let mut graphics = body(0.8, 1.45, 2.05, 1.95);
    graphics.push(line("TopSilkscreen", p(-0.4, -1.56), p(0.4, -1.56)));
    Part {
        id: "SOT-23-5",
        pads: vec![
            smd("1", -0.95, 1.1, 0.6, 1.1),
            smd("2", 0.0, 1.1, 0.6, 1.1),
            smd("3", 0.95, 1.1, 0.6, 1.1),
            smd("4", 0.95, -1.1, 0.6, 1.1),
            smd("5", -0.95, -1.1, 0.6, 1.1),
        ],
        graphics,
        label_at: p(0.0, -2.8),
    }
}

/// A single-row 2.54 mm pin header along +y; pin 1 square.
pub fn pin_header(pins: usize) -> Part {
    let ids = [
        "PinHeader_1x02_P2.54mm",
        "PinHeader_1x03_P2.54mm",
        "PinHeader_1x04_P2.54mm",
        "PinHeader_1x05_P2.54mm",
        "PinHeader_1x06_P2.54mm",
        "PinHeader_1x07_P2.54mm",
        "PinHeader_1x08_P2.54mm",
    ];
    let len = 2.54 * (pins - 1) as f64;
    let mut pads = Vec::new();
    for i in 0..pins {
        let shape = if i == 0 {
            PadShape::Rect
        } else {
            PadShape::Oval
        };
        pads.push(tht(
            &(i + 1).to_string(),
            shape,
            0.0,
            2.54 * i as f64,
            1.7,
            1.0,
        ));
    }
    let mut graphics = vec![
        FpGraphic {
            start: Some(p(-1.27, -1.27)),
            end: Some(p(1.27, len + 1.27)),
            ..graphic("rect", "TopFab", 0.1)
        },
        FpGraphic {
            points: vec![
                p(-1.8, -1.8),
                p(1.8, -1.8),
                p(1.8, len + 1.8),
                p(-1.8, len + 1.8),
            ],
            ..graphic("poly", "TopCourtyard", 0.05)
        },
    ];
    graphics.push(line("TopSilkscreen", p(-1.33, 1.27), p(-1.33, len + 1.33)));
    graphics.push(line("TopSilkscreen", p(1.33, 1.27), p(1.33, len + 1.33)));
    graphics.push(line(
        "TopSilkscreen",
        p(-1.33, len + 1.33),
        p(1.33, len + 1.33),
    ));
    graphics.push(line("TopSilkscreen", p(-1.33, 0.0), p(-1.33, -1.33)));
    graphics.push(line("TopSilkscreen", p(-1.33, -1.33), p(0.0, -1.33)));
    Part {
        id: ids[pins - 2],
        pads,
        graphics,
        label_at: p(0.0, -2.6),
    }
}

/// A power-only USB-C receptacle (6 contacts) with four shell slots.
/// The contacts sit along the plug opening at `-y`; the shell tabs
/// are plated slots, all pin `S1`.
pub fn usb_c_6p() -> Part {
    let contacts = [
        ("A12", -3.35, 0.7),
        ("A9", -2.25, 0.7),
        ("A5", -0.5, 0.6),
        ("B5", 0.5, 0.6),
        ("B9", 2.25, 0.7),
        ("B12", 3.35, 0.7),
    ];
    let mut pads: Vec<Pad> = contacts
        .iter()
        .map(|&(n, x, w)| smd(n, x, -1.6, w, 1.4))
        .collect();
    for (x, y) in [(-4.32, -1.0), (4.32, -1.0), (-4.32, 2.8), (4.32, 2.8)] {
        pads.push(slot("S1", x, y, 1.0, 2.1, 0.6));
    }
    let mut graphics = body(4.47, 3.65, 5.32, 3.95);
    graphics.push(line("TopSilkscreen", p(-4.57, 0.6), p(-4.57, 1.6)));
    graphics.push(line("TopSilkscreen", p(4.57, 0.6), p(4.57, 1.6)));
    graphics.push(line("TopSilkscreen", p(-3.4, 3.75), p(3.4, 3.75)));
    Part {
        id: "USB-C_6P_PowerOnly",
        pads,
        graphics,
        label_at: p(0.0, 5.0),
    }
}

/// TQFP-32, 7 x 7 mm body, 0.8 mm pitch; pin 1 top-left, counter-clockwise
/// as seen from the top: left side down, bottom side right, right side up,
/// top side left.
pub fn tqfp32() -> Part {
    let mut pads = Vec::new();
    for k in 0..8 {
        let o = -2.8 + 0.8 * f64::from(k);
        pads.push(smd(&(k + 1).to_string(), -4.25, o, 1.6, 0.55));
        pads.push(smd(&(k + 9).to_string(), o, 4.25, 0.55, 1.6));
        pads.push(smd(&(k + 17).to_string(), 4.25, -o, 1.6, 0.55));
        pads.push(smd(&(k + 25).to_string(), -o, -4.25, 0.55, 1.6));
    }
    pads.sort_by_key(|pad| pad.number.parse::<u32>().unwrap_or(0));
    let mut graphics = body(3.5, 3.5, 5.3, 5.3);
    for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        graphics.push(line(
            "TopSilkscreen",
            p(sx * 3.61, sy * 3.61),
            p(sx * 3.2, sy * 3.61),
        ));
        graphics.push(line(
            "TopSilkscreen",
            p(sx * 3.61, sy * 3.61),
            p(sx * 3.61, sy * 3.2),
        ));
    }
    graphics.push(circle("TopSilkscreen", p(-2.7, -2.7), 0.25, 0.12));
    Part {
        id: "TQFP-32",
        pads,
        graphics,
        label_at: p(0.0, 0.0),
    }
}

/// A two-pad 5.0 x 3.2 mm SMD crystal.
pub fn crystal_5032() -> Part {
    two_terminal("Crystal_5032_2Pin", 1.85, 2.0, 2.4, 5.0, 3.2)
}

/// An 8-pad 2.5 x 2.5 mm LGA environmental sensor; pins 1-4 down the left,
/// 5-8 up the right.
pub fn lga8_2p5() -> Part {
    let mut pads = Vec::new();
    for k in 0..4 {
        let y = -0.975 + 0.65 * f64::from(k);
        pads.push(smd(&(k + 1).to_string(), -1.0, y, 0.5, 0.35));
        pads.push(smd(&(8 - k).to_string(), 1.0, y, 0.5, 0.35));
    }
    pads.sort_by_key(|pad| pad.number.parse::<u32>().unwrap_or(0));
    let mut graphics = body(1.25, 1.25, 1.65, 1.65);
    graphics.push(circle("TopSilkscreen", p(-1.6, -1.5), 0.1, 0.2));
    Part {
        id: "LGA-8_2.5x2.5mm",
        pads,
        graphics,
        label_at: p(0.0, -2.4),
    }
}

/// A 5.5/2.1 mm barrel jack, horizontal, with plated slots: pin 1 the centre
/// pin, pin 2 the sleeve, pin 3 the sleeve switch.
pub fn barrel_jack() -> Part {
    // Centre and sleeve slots run across x; the switch slot runs along y.
    let pads = vec![
        slot("1", 0.0, 0.0, 4.0, 2.0, 1.2),
        slot("2", 0.0, -6.0, 4.0, 2.0, 1.0),
        slot("3", 4.7, -3.0, 2.0, 3.5, 1.0),
    ];
    let mut graphics = vec![
        FpGraphic {
            start: Some(p(-4.5, -13.7)),
            end: Some(p(4.5, 0.8)),
            ..graphic("rect", "TopFab", 0.1)
        },
        FpGraphic {
            points: vec![p(-5.0, -14.2), p(6.5, -14.2), p(6.5, 2.5), p(-5.0, 2.5)],
            ..graphic("poly", "TopCourtyard", 0.05)
        },
    ];
    graphics.push(line("TopSilkscreen", p(-4.6, -13.8), p(4.6, -13.8)));
    graphics.push(line("TopSilkscreen", p(-4.6, -13.8), p(-4.6, -1.5)));
    graphics.push(line("TopSilkscreen", p(4.6, -13.8), p(4.6, -5.0)));
    Part {
        id: "DC_Jack_5.5x2.1mm",
        pads,
        graphics,
        label_at: p(0.0, 2.8),
    }
}

/// A 2-way 5.08 mm screw terminal block.
pub fn screw_terminal_2() -> Part {
    let pads = vec![
        tht("1", PadShape::Rect, 0.0, 0.0, 2.6, 1.3),
        tht("2", PadShape::Circle, 5.08, 0.0, 2.6, 1.3),
    ];
    let mut graphics = vec![
        FpGraphic {
            start: Some(p(-2.54, -4.6)),
            end: Some(p(7.62, 4.6)),
            ..graphic("rect", "TopFab", 0.1)
        },
        FpGraphic {
            points: vec![p(-3.0, -5.1), p(8.1, -5.1), p(8.1, 5.1), p(-3.0, 5.1)],
            ..graphic("poly", "TopCourtyard", 0.05)
        },
    ];
    for (a, b) in [
        (p(-2.66, -4.72), p(7.74, -4.72)),
        (p(7.74, -4.72), p(7.74, 4.72)),
        (p(7.74, 4.72), p(-2.66, 4.72)),
        (p(-2.66, 4.72), p(-2.66, -4.72)),
    ] {
        graphics.push(line("TopSilkscreen", a, b));
    }
    Part {
        id: "Terminal_2P_5.08mm",
        pads,
        graphics,
        label_at: p(2.54, -5.8),
    }
}

/// A radial electrolytic capacitor, 6.3 mm can, 2.5 mm lead pitch; pin 1
/// is the plus lead.
pub fn cap_radial_6p3() -> Part {
    let pads = vec![
        tht("1", PadShape::Rect, 0.0, 0.0, 1.6, 0.8),
        tht("2", PadShape::Circle, 2.5, 0.0, 1.6, 0.8),
    ];
    let c = p(1.25, 0.0);
    let mut graphics = vec![
        circle("TopFab", c, 3.15, 0.1),
        circle("TopCourtyard", c, 3.4, 0.05),
        circle("TopSilkscreen", c, 3.27, 0.12),
    ];
    graphics.push(line("TopSilkscreen", p(-1.8, -1.6), p(-1.0, -1.6)));
    graphics.push(line("TopSilkscreen", p(-1.4, -2.0), p(-1.4, -1.2)));
    Part {
        id: "CP_Radial_6.3x2.5mm",
        pads,
        graphics,
        label_at: p(1.25, -4.3),
    }
}

/// TO-220-3 standing up; pins 1-3 in a row at 2.54 mm, oval pads.
pub fn to220_vertical() -> Part {
    let mut pads = Vec::new();
    for k in 0..3 {
        let shape = if k == 0 {
            PadShape::Rect
        } else {
            PadShape::Oval
        };
        let mut pad = tht(
            &(k + 1).to_string(),
            shape,
            2.54 * f64::from(k),
            0.0,
            1.9,
            1.1,
        );
        pad.size = p(1.9, 2.6);
        pads.push(pad);
    }
    let mut graphics = vec![
        FpGraphic {
            start: Some(p(-2.46, -3.27)),
            end: Some(p(7.54, 1.4)),
            ..graphic("rect", "TopFab", 0.1)
        },
        FpGraphic {
            points: vec![p(-2.7, -3.5), p(7.8, -3.5), p(7.8, 1.9), p(-2.7, 1.9)],
            ..graphic("poly", "TopCourtyard", 0.05)
        },
    ];
    graphics.push(line("TopSilkscreen", p(-2.58, -3.39), p(7.66, -3.39)));
    graphics.push(line("TopSilkscreen", p(-2.58, -1.88), p(7.66, -1.88)));
    Part {
        id: "TO-220_Vertical",
        pads,
        graphics,
        label_at: p(2.54, -4.4),
    }
}
