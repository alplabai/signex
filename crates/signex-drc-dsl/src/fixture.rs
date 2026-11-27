//! The base rules (#2474) as one rule text and, for each rule, a board that
//! passes it and one that breaks it. Shared by this crate's tests and the
//! app's `check_drc` and Run DRC tests, so every layer checks the same
//! cases. Each board is clean under every other base rule, so a broken
//! board's findings are of its rule alone.

use signex_types::pcb::{
    BoardGraphic, DrillDef, Footprint, FpGraphic, NetDef, Pad, PadNet, PadShape, PadType, PcbBoard,
    Point, Segment, Via, ViaType,
};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_types::pcb_route_attrs::MaskExpansion;
use signex_types::pcb_sketch::{BoardSketch, PcbKeepout};
use signex_types::project::SetupProfile;
use signex_types::sketch::attr::KeepoutKinds;
use uuid::Uuid;

/// One rule text holding every written base rule.
pub const BASE_RULES: &str = "\
rule clearance on clearance require clearance >= 0.2mm then error
rule widths on track require width >= 0.2mm and width <= 1mm then error
rule holes on hole require diameter >= 0.3mm and diameter <= 3mm and annular >= 0.15mm \
and spacing >= 0.5mm then error
rule no_via_in_pad on via require pad_gap >= 0mm then error
rule edge on edge require clearance >= 0.5mm then error
rule silk on silk require pad_clearance >= 0.1mm and spacing >= 0.1mm then warning
rule mask on mask require sliver >= 0.1mm and expansion >= 0mm and expansion <= 0.1mm \
then warning
rule paste on paste require expansion >= 0mm and expansion <= 0.05mm then warning
rule courtyard on courtyard require spacing >= 0mm then error
";

/// The 15 base rules, in the order `docs/ROADMAP.md` lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseRule {
    Clearance,
    TrackWidth,
    AnnularRing,
    HoleSize,
    HoleToHole,
    ViaInPad,
    SilkToPad,
    SilkToSilk,
    Mask,
    Paste,
    EdgeClearance,
    Keepout,
    Unrouted,
    Short,
    Courtyard,
}

impl BaseRule {
    pub const ALL: [BaseRule; 15] = [
        BaseRule::Clearance,
        BaseRule::TrackWidth,
        BaseRule::AnnularRing,
        BaseRule::HoleSize,
        BaseRule::HoleToHole,
        BaseRule::ViaInPad,
        BaseRule::SilkToPad,
        BaseRule::SilkToSilk,
        BaseRule::Mask,
        BaseRule::Paste,
        BaseRule::EdgeClearance,
        BaseRule::Keepout,
        BaseRule::Unrouted,
        BaseRule::Short,
        BaseRule::Courtyard,
    ];

    /// The built-in id of the kind of finding a broken board gives
    /// (`builtin::<kind>`, also for a finding of a written rule).
    pub const fn kind_id(self) -> &'static str {
        match self {
            BaseRule::Clearance => "builtin::clearance",
            BaseRule::TrackWidth => "builtin::track_width",
            BaseRule::AnnularRing => "builtin::annular_ring",
            BaseRule::HoleSize => "builtin::hole_size",
            BaseRule::HoleToHole => "builtin::hole_to_hole",
            BaseRule::ViaInPad => "builtin::via_in_pad",
            BaseRule::SilkToPad => "builtin::silk_to_pad",
            BaseRule::SilkToSilk => "builtin::silk_to_silk",
            BaseRule::Mask => "builtin::mask_sliver",
            BaseRule::Paste => "builtin::paste_expansion",
            BaseRule::EdgeClearance => "builtin::edge_clearance",
            BaseRule::Keepout => "builtin::keepout",
            BaseRule::Unrouted => "builtin::unconnected",
            BaseRule::Short => "builtin::short",
            BaseRule::Courtyard => "builtin::courtyard_overlap",
        }
    }

    /// The rule a broken board's finding is filed under: the written rule
    /// of [`BASE_RULES`] that sets the limit, or the built-in rule.
    pub const fn rule_id(self) -> &'static str {
        match self {
            BaseRule::Clearance => "user::clearance",
            BaseRule::TrackWidth => "user::widths",
            BaseRule::AnnularRing | BaseRule::HoleSize | BaseRule::HoleToHole => "user::holes",
            BaseRule::ViaInPad => "user::no_via_in_pad",
            BaseRule::SilkToPad | BaseRule::SilkToSilk => "user::silk",
            BaseRule::Mask => "user::mask",
            BaseRule::Paste => "user::paste",
            BaseRule::EdgeClearance => "user::edge",
            BaseRule::Courtyard => "user::courtyard",
            BaseRule::Keepout | BaseRule::Unrouted | BaseRule::Short => self.kind_id(),
        }
    }

    /// The board for this rule: `broken` breaks it, else it passes.
    pub fn board(self, broken: bool) -> PcbBoard {
        let mut b = blank();
        let pick = |pass: f64, fail: f64| if broken { fail } else { pass };
        match self {
            BaseRule::Clearance => {
                b.segments = vec![
                    track(1, (20.0, 25.0), (30.0, 25.0), 1),
                    track(
                        2,
                        (20.0, 25.0 + pick(0.5, 0.4)),
                        (30.0, 25.0 + pick(0.5, 0.4)),
                        2,
                    ),
                ];
            }
            BaseRule::TrackWidth => {
                let mut t = track(1, (20.0, 25.0), (30.0, 25.0), 1);
                t.width = pick(0.25, 0.15);
                b.segments = vec![t];
            }
            BaseRule::AnnularRing => {
                let mut v = via(1, (25.0, 25.0), 1);
                v.diameter = pick(0.6, 0.55);
                b.vias = vec![v];
            }
            BaseRule::HoleSize => {
                let mut p = pad(1, (0.0, 0.0), Some(3), PadType::Thru);
                p.size = Point::new(4.0, 4.0);
                p.drill = Some(DrillDef {
                    diameter: pick(2.0, 3.5),
                    shape: String::new(),
                });
                b.footprints = vec![footprint(1, (25.0, 25.0), vec![p], Vec::new())];
            }
            BaseRule::HoleToHole => {
                b.vias = vec![
                    via(1, (25.0, 25.0), 1),
                    via(2, (25.0 + pick(1.0, 0.75), 25.0), 1),
                ];
            }
            BaseRule::ViaInPad => {
                let p = pad(1, (0.0, 0.0), Some(1), PadType::Smd);
                b.footprints = vec![footprint(1, (25.0, 25.0), vec![p], Vec::new())];
                b.vias = vec![via(1, (pick(27.0, 25.0), 25.0), 1)];
            }
            BaseRule::SilkToPad => {
                let p = pad(1, (0.0, 0.0), None, PadType::Smd);
                let y = pick(0.8, 0.55);
                let silk = line("TopSilkscreen", (-1.0, y), (1.0, y));
                b.footprints = vec![footprint(1, (25.0, 25.0), vec![p], vec![silk])];
            }
            BaseRule::SilkToSilk => {
                let a = line("TopSilkscreen", (0.0, 0.0), (2.0, 0.0));
                let b_line = line("TopSilkscreen", (0.0, 0.0), (2.0, 0.0));
                let gap = pick(0.6, 0.15);
                b.footprints = vec![
                    footprint(1, (25.0, 25.0), Vec::new(), vec![a]),
                    footprint(2, (25.0, 25.0 + gap), Vec::new(), vec![b_line]),
                ];
                // A board frame drawn line by line touches itself at its
                // corner; board drawings are not checked against each other.
                b.graphics = vec![
                    board_line(1, (5.0, 5.0), (10.0, 5.0)),
                    board_line(2, (10.0, 5.0), (10.0, 10.0)),
                ];
            }
            BaseRule::Mask => {
                let pitch = pick(1.3, 1.05);
                let pads = vec![
                    pad(1, (0.0, 0.0), None, PadType::Smd),
                    pad(2, (pitch, 0.0), None, PadType::Smd),
                ];
                b.footprints = vec![footprint(1, (25.0, 25.0), pads, Vec::new())];
            }
            BaseRule::Paste => {
                let mut t = track(1, (20.0, 25.0), (30.0, 25.0), 1);
                t.attributes.paste_mask = MaskExpansion::Manual {
                    mm: pick(0.02, 0.1),
                };
                b.segments = vec![t];
            }
            BaseRule::EdgeClearance => {
                let x = pick(5.0, 0.2);
                b.segments = vec![track(1, (x, 20.0), (x, 30.0), 1)];
            }
            BaseRule::Keepout => {
                let y = pick(10.0, 25.0);
                b.segments = vec![track(1, (15.0, y), (35.0, y), 1)];
                b.sketch = Some(keepout());
            }
            BaseRule::Unrouted => {
                let pads = vec![
                    pad(1, (-5.0, 0.0), Some(1), PadType::Smd),
                    pad(2, (5.0, 0.0), Some(1), PadType::Smd),
                ];
                b.footprints = vec![footprint(1, (25.0, 25.0), pads, Vec::new())];
                if !broken {
                    b.segments = vec![track(1, (20.0, 25.0), (30.0, 25.0), 1)];
                }
            }
            BaseRule::Short => {
                b.segments = vec![
                    track(1, (20.0, 25.0), (30.0, 25.0), 1),
                    track(2, (25.0, pick(30.0, 20.0)), (25.0, 35.0), 2),
                ];
            }
            BaseRule::Courtyard => {
                let yard = || {
                    vec![FpGraphic {
                        graphic_type: "rect".into(),
                        layer: "TopCourtyard".into(),
                        width: 0.05,
                        start: Some(Point::new(-1.0, -1.0)),
                        end: Some(Point::new(1.0, 1.0)),
                        center: None,
                        mid: None,
                        radius: 0.0,
                        points: Vec::new(),
                        text: String::new(),
                        font_size: 0.0,
                        position: None,
                        rotation: 0.0,
                        fill: String::new(),
                    }]
                };
                b.footprints = vec![
                    footprint(1, (25.0, 25.0), Vec::new(), yard()),
                    footprint(2, (25.0 + pick(3.0, 1.5), 25.0), Vec::new(), yard()),
                ];
            }
        }
        b
    }

    /// Boards that break this rule another way than [`BaseRule::board`],
    /// each with the built-in id of the kind of finding it gives: a track
    /// over the maximum width, a via mask expansion over the maximum.
    pub fn also_broken(self) -> Vec<(PcbBoard, &'static str)> {
        match self {
            BaseRule::TrackWidth => {
                let mut b = blank();
                let mut t = track(1, (20.0, 25.0), (30.0, 25.0), 1);
                t.width = 1.2;
                b.segments = vec![t];
                vec![(b, "builtin::track_width")]
            }
            BaseRule::Mask => {
                let mut b = blank();
                let mut v = via(1, (25.0, 25.0), 1);
                v.attributes.top.expansion = MaskExpansion::Manual { mm: 0.2 };
                v.attributes.bottom.expansion = MaskExpansion::Manual { mm: 0.2 };
                b.vias = vec![v];
                vec![(b, "builtin::mask_expansion")]
            }
            _ => Vec::new(),
        }
    }
}

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// A two-layer board, 50 x 50 mm, nets 1 `GND`, 2 `VCC`, 3 `SIG`.
fn blank() -> PcbBoard {
    let mut b = SetupProfile::Standard.pcb(2).new_board();
    b.uuid = id(0xB0A2D);
    b.outline = [(0.0, 0.0), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)]
        .into_iter()
        .map(|(x, y)| Point::new(x, y))
        .collect();
    b.nets = [(1, "GND"), (2, "VCC"), (3, "SIG")]
        .into_iter()
        .map(|(number, name)| NetDef {
            number,
            name: name.into(),
        })
        .collect();
    b
}

fn track(n: u128, a: (f64, f64), b: (f64, f64), net: u32) -> Segment {
    Segment {
        locked: false,
        attributes: Default::default(),
        uuid: id(0x2000 + n),
        start: Point::new(a.0, a.1),
        end: Point::new(b.0, b.1),
        width: 0.25,
        layer: "TopCopper".into(),
        net,
        unknown_columns: Default::default(),
    }
}

fn via(n: u128, at: (f64, f64), net: u32) -> Via {
    Via {
        attributes: Default::default(),
        uuid: id(0x3000 + n),
        position: Point::new(at.0, at.1),
        diameter: 0.6,
        drill: 0.3,
        layers: vec!["TopCopper".into(), "BottomCopper".into()],
        net,
        locked: false,
        layer_sizes: None,
        via_type: ViaType::Through,
        unknown_columns: Default::default(),
    }
}

/// A 1 x 1 mm pad at `at` in its footprint.
fn pad(n: u128, at: (f64, f64), net: Option<u32>, pad_type: PadType) -> Pad {
    let names = ["", "GND", "VCC", "SIG"];
    Pad {
        uuid: id(0x4000 + n),
        number: n.to_string(),
        pad_type,
        shape: PadShape::Rect,
        position: Point::new(at.0, at.1),
        size: Point::new(1.0, 1.0),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: net.map(|number| PadNet {
            number,
            name: names[number as usize].into(),
        }),
        roundrect_ratio: 0.0,
        rotation: 0.0,
        trapezoid_delta: Default::default(),
        custom_outline: Vec::new(),
        layer_sizes: None,
        net_locked: false,
        thermal: Default::default(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn footprint(n: u128, at: (f64, f64), pads: Vec<Pad>, graphics: Vec<FpGraphic>) -> Footprint {
    Footprint {
        uuid: id(0x1000 + n),
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
        pads,
        graphics,
        properties: Vec::new(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

/// A 0.1 mm silkscreen line on `layer`, in its footprint.
fn line(layer: &str, a: (f64, f64), b: (f64, f64)) -> FpGraphic {
    FpGraphic {
        graphic_type: "line".into(),
        layer: layer.into(),
        width: 0.1,
        start: Some(Point::new(a.0, a.1)),
        end: Some(Point::new(b.0, b.1)),
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

/// A 0.1 mm board-level line on the top silkscreen.
fn board_line(n: u128, a: (f64, f64), b: (f64, f64)) -> BoardGraphic {
    BoardGraphic {
        uuid: id(0x5000 + n),
        graphic_type: "line".into(),
        layer: "TopSilkscreen".into(),
        width: 0.1,
        start: Some(Point::new(a.0, a.1)),
        end: Some(Point::new(b.0, b.1)),
        center: None,
        radius: 0.0,
        points: Vec::new(),
        text: String::new(),
        font_size: 0.0,
    }
}

/// A routing keepout on the top copper, 10 x 10 mm at (20, 20).
fn keepout() -> BoardSketch {
    let corners = [(20.0, 20.0), (30.0, 20.0), (30.0, 30.0), (20.0, 30.0)];
    BoardSketch {
        keepouts: vec![PcbKeepout {
            uuid: id(0x900),
            boundary: corners.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            layers: vec![PcbLayer::Copper(CopperLayer::Top)],
            forbids: KeepoutKinds {
                no_routing: true,
                ..KeepoutKinds::default()
            },
        }],
        ..BoardSketch::default()
    }
}
