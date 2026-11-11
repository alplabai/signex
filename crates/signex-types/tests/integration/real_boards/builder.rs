//! A small deterministic board builder for the realistic corpus boards.
//!
//! Every uuid comes from a per-board counter, so a run writes the same bytes
//! every time. Coordinates are board millimetres. The world transform of a
//! pad (mirror x on the bottom, turn counter-clockwise in the model's axes,
//! then translate) is the one `pcb-geom` uses, so a track routed to
//! [`Builder::pad_at`] ends on the pad centre the geometry crate computes.

use signex_types::format::{parse_pcb_layer, pcb_layer_token};
use signex_types::net_class::NetClass;
use signex_types::pcb::{
    BoardGraphic, BoardText, Footprint, FpGraphic, LayerDef, NetDef, Pad, PadNet, PcbBoard,
    PcbSetup, Point, Segment, SymbolLink, Via, ViaType, Zone,
};
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};
use signex_types::pcb_sketch::BoardSketch;
use signex_types::project::PcbDefaults;
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use super::parts::Part;

pub const TOP: &str = "TopCopper";
pub const BOTTOM: &str = "BottomCopper";
pub const IN1: &str = "Inner1Copper";
pub const IN2: &str = "Inner2Copper";

pub fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// Which face a footprint is placed on.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Top,
    Bottom,
}

/// A board being built, plus the project net classes that go with it.
pub struct Builder {
    board: PcbBoard,
    board_no: u128,
    seq: u128,
    pub net_classes: Vec<NetClass>,
}

/// The board-wide rules written to `[stackup.setup]`.
pub struct Rules {
    pub copper_layers: u8,
    pub clearance: f64,
    pub track: f64,
    pub via_diameter: f64,
    pub via_drill: f64,
}

impl Builder {
    pub fn new(board_no: u8, rules: Rules) -> Self {
        let mut board = PcbDefaults {
            copper_layers: rules.copper_layers,
            thickness_mm: 1.6,
            clearance_mm: rules.clearance,
            track_width_mm: rules.track,
            via_diameter_mm: rules.via_diameter,
            via_drill_mm: rules.via_drill,
        }
        .new_board();
        // The new-board stack lacks the courtyard and fab layers the
        // footprints draw on; list them so every graphic has its layer.
        for technical in [
            TechnicalLayer::CourtyardTop,
            TechnicalLayer::CourtyardBottom,
            TechnicalLayer::FabTop,
            TechnicalLayer::FabBottom,
        ] {
            let id = u8::try_from(board.layers.len()).expect("few layers");
            board.layers.push(LayerDef {
                id,
                name: pcb_layer_token(PcbLayer::Technical(technical)),
                layer_type: String::new(),
                display_name: String::new(),
            });
        }
        board.setup = Some(PcbSetup {
            grid_size: 0.05,
            trace_width: rules.track,
            via_diameter: rules.via_diameter,
            via_drill: rules.via_drill,
            clearance: rules.clearance,
            track_min_width: 0.15,
            via_min_diameter: 0.45,
            via_min_drill: 0.2,
        });
        let mut builder = Builder {
            board,
            board_no: u128::from(board_no),
            seq: 0,
            net_classes: Vec::new(),
        };
        builder.board.uuid = builder.uuid();
        builder
    }

    /// The next uuid of this board: `5ca1ab1e-00BB-4000-8000-0000000SSSSS`.
    pub fn uuid(&mut self) -> Uuid {
        self.seq += 1;
        Uuid::from_u128(0x5ca1_ab1e_0000_4000_8000_0000_0000_0000 | self.board_no << 80 | self.seq)
    }

    /// Name the stack's copper layers for display, e.g. `GND plane`.
    pub fn layer_display_name(&mut self, layer: &str, name: &str) {
        let def = self
            .board
            .layers
            .iter_mut()
            .find(|l| l.name == layer)
            .expect("layer in stack");
        def.display_name = name.to_string();
    }

    /// Mark a copper layer as a plane layer in the stack's type column.
    pub fn plane_layer(&mut self, layer: &str) {
        let def = self
            .board
            .layers
            .iter_mut()
            .find(|l| l.name == layer)
            .expect("layer in stack");
        def.layer_type = "plane".into();
    }

    /// The number of net `name`, added to the net table on first use.
    pub fn net(&mut self, name: &str) -> u32 {
        if let Some(net) = self.board.nets.iter().find(|n| n.name == name) {
            return net.number;
        }
        let number = u32::try_from(self.board.nets.len()).expect("few nets") + 1;
        self.board.nets.push(NetDef {
            number,
            name: name.to_string(),
        });
        number
    }

    pub fn net_class(&mut self, class: NetClass) {
        self.net_classes.push(class);
    }

    /// The closed board outline (closing vertex not repeated), also drawn on
    /// `EdgeCuts` as line segments so the mechanical layer carries it.
    pub fn outline(&mut self, points: Vec<Point>) {
        for (i, &a) in points.iter().enumerate() {
            self.edge(
                "line",
                Some(a),
                Some(points[(i + 1) % points.len()]),
                None,
                0.0,
            );
        }
        self.board.outline = points;
    }

    /// The board outline polygon only; the caller draws its `EdgeCuts`
    /// edges with [`Self::edge`] (an arc as one arc, not its chords).
    pub fn outline_polygon(&mut self, points: Vec<Point>) {
        self.board.outline = points;
    }

    /// One `EdgeCuts` graphic: a `line` from `start` to `end`, or an `arc`
    /// about `center` from `start` to `end`.
    pub fn edge(
        &mut self,
        kind: &str,
        start: Option<Point>,
        end: Option<Point>,
        center: Option<Point>,
        radius: f64,
    ) {
        self.board.graphics.push(BoardGraphic {
            graphic_type: kind.into(),
            layer: "EdgeCuts".into(),
            width: 0.05,
            start,
            end,
            center,
            radius,
            points: Vec::new(),
        });
    }

    pub fn board_text(&mut self, text: &str, at: Point, layer: &str, size: f64, rotation: f64) {
        let uuid = self.uuid();
        self.board.texts.push(BoardText {
            uuid,
            text: text.to_string(),
            position: at,
            layer: layer.to_string(),
            font_size: size,
            rotation,
        });
    }

    /// Place `part` and give its pads the nets `nets` lists by pin number
    /// (`(pin, net)`); a pin not listed has no net.
    #[allow(clippy::too_many_arguments)]
    pub fn place(
        &mut self,
        part: Part,
        reference: &str,
        value: &str,
        at: Point,
        rotation: f64,
        side: Side,
        nets: &[(&str, &str)],
    ) -> usize {
        let fp_uuid = self.uuid();
        let mut pads = Vec::with_capacity(part.pads.len());
        for def in part.pads {
            let net = nets
                .iter()
                .find(|(pin, _)| *pin == def.number)
                .map(|(_, name)| PadNet {
                    number: self.net(name),
                    name: name.to_string(),
                });
            let uuid = self.uuid();
            pads.push(Pad { uuid, net, ..def });
        }
        let mut graphics = part.graphics;
        graphics.push(FpGraphic {
            graphic_type: "text".into(),
            layer: "TopSilkscreen".into(),
            width: 0.15,
            start: None,
            end: None,
            center: None,
            mid: None,
            radius: 0.0,
            points: Vec::new(),
            text: reference.to_string(),
            font_size: 1.0,
            position: Some(part.label_at),
            rotation: 0.0,
            fill: String::new(),
        });
        let mut footprint = Footprint {
            uuid: fp_uuid,
            reference: reference.to_string(),
            value: value.to_string(),
            footprint_id: part.id.to_string(),
            position: at,
            rotation,
            layer: TOP.into(),
            locked: false,
            pads,
            graphics,
            properties: Vec::new(),
            schematic_link: None,
            library_ref: None,
            unknown: Default::default(),
            unknown_columns: Default::default(),
        };
        if side == Side::Bottom {
            flip_layers(&mut footprint);
        }
        self.board.footprints.push(footprint);
        self.board.footprints.len() - 1
    }

    /// Link footprint `index` to schematic symbol `unit` on the root sheet.
    pub fn link(&mut self, index: usize, unit: Uuid) {
        self.board.footprints[index].schematic_link =
            Some(SymbolLink::new(InstancePath::root(), [unit]));
    }

    pub fn footprint_ref(&self, index: usize) -> &str {
        &self.board.footprints[index].reference
    }

    /// Lock footprint `index` in place (a mounting part or connector).
    pub fn lock(&mut self, index: usize) {
        self.board.footprints[index].locked = true;
    }

    /// The board position of pad `pin` of footprint `index`.
    pub fn pad_at(&self, index: usize, pin: &str) -> Point {
        let fp = &self.board.footprints[index];
        let pad = fp
            .pads
            .iter()
            .find(|pad| pad.number == pin)
            .unwrap_or_else(|| panic!("{} has no pad {pin}", fp.reference));
        pad_world(fp, pad)
    }

    /// A routed track through `points`, one segment per leg.
    pub fn track(&mut self, net: &str, layer: &str, width: f64, points: &[Point]) {
        let net = self.net(net);
        for leg in points.windows(2) {
            let uuid = self.uuid();
            self.board.segments.push(Segment {
                uuid,
                start: leg[0],
                end: leg[1],
                width,
                layer: layer.to_string(),
                net,
                unknown_columns: Default::default(),
            });
        }
    }

    /// A through via.
    pub fn via(&mut self, net: &str, at: Point, diameter: f64, drill: f64) {
        let copper: Vec<&String> = self
            .board
            .layers
            .iter()
            .filter(|l| {
                parse_pcb_layer(&l.name)
                    .and_then(PcbLayer::copper)
                    .is_some()
            })
            .map(|l| &l.name)
            .collect();
        let layers = vec![copper[0].clone(), copper[copper.len() - 1].clone()];
        let net = self.net(net);
        let uuid = self.uuid();
        self.board.vias.push(Via {
            uuid,
            position: at,
            diameter,
            drill,
            layers,
            net,
            via_type: ViaType::Through,
            unknown_columns: Default::default(),
        });
    }

    /// A copper pour with thermal reliefs.
    pub fn zone(
        &mut self,
        net: &str,
        layer: &str,
        outline: Vec<Point>,
        holes: Vec<Vec<Point>>,
        priority: u32,
    ) {
        let number = self.net(net);
        let uuid = self.uuid();
        self.board.zones.push(Zone {
            uuid,
            net: number,
            net_name: net.to_string(),
            layer: layer.to_string(),
            outline,
            holes,
            priority,
            fill_type: "solid".into(),
            thermal_relief: true,
            thermal_gap: 0.5,
            thermal_width: 0.3,
            clearance: 0.3,
            min_thickness: 0.254,
            unknown: Default::default(),
        });
    }

    /// A plane layer pour of `net`: one zone over `outline` with a square
    /// antipad (`hole` mm across) around every via and through-hole pad of
    /// another net, so the plane clears what passes through it.
    pub fn plane(
        &mut self,
        net: &str,
        layer: &str,
        outline: Vec<Point>,
        via_hole: f64,
        pad_hole: f64,
    ) {
        let number = self.net(net);
        let mut holes = Vec::new();
        let square = |c: Point, side: f64| {
            let h = side / 2.0;
            rect(c.x - h, c.y - h, c.x + h, c.y + h)
        };
        for via in self.board.vias.iter().filter(|v| v.net != number) {
            holes.push(square(via.position, via_hole));
        }
        for fp in &self.board.footprints {
            for pad in &fp.pads {
                let foreign = pad.net.as_ref().is_none_or(|n| n.number != number);
                if pad.pad_type == signex_types::pcb::PadType::Thru && foreign {
                    holes.push(square(pad_world(fp, pad), pad_hole));
                }
            }
        }
        self.zone(net, layer, outline, holes, 0);
    }

    pub fn sketch(&mut self, sketch: BoardSketch) {
        self.board.sketch = Some(sketch);
    }

    pub fn finish(self) -> (PcbBoard, Vec<NetClass>) {
        (self.board, self.net_classes)
    }
}

/// Put a footprint built for the top on the bottom: every layer to its twin.
/// Local coordinates stay; the world transform mirrors them.
fn flip_layers(footprint: &mut Footprint) {
    let flip = |layer: &mut String| {
        if let Some(parsed) = parse_pcb_layer(layer) {
            *layer = pcb_layer_token(parsed.flipped());
        }
    };
    flip(&mut footprint.layer);
    for pad in &mut footprint.pads {
        pad.layers.iter_mut().for_each(flip);
    }
    for graphic in &mut footprint.graphics {
        flip(&mut graphic.layer);
    }
}

/// The board position of `pad`'s centre, snapped to the nm grid the file
/// stores so a track end equals the centre read back.
pub fn pad_world(fp: &Footprint, pad: &Pad) -> Point {
    to_world(fp, pad.position)
}

/// A footprint-local point on the board.
pub fn to_world(fp: &Footprint, local: Point) -> Point {
    let local = if fp.layer == BOTTOM {
        p(-local.x, local.y)
    } else {
        local
    };
    let (sin, cos) = sin_cos_deg(fp.rotation);
    let x = local.x * cos - local.y * sin + fp.position.x;
    let y = local.x * sin + local.y * cos + fp.position.y;
    p(snap(x), snap(y))
}

/// Exact for the quarter turns, as in `pcb-geom`.
fn sin_cos_deg(deg: f64) -> (f64, f64) {
    let turn = deg.rem_euclid(360.0);
    if turn == 0.0 {
        (0.0, 1.0)
    } else if turn == 90.0 {
        (1.0, 0.0)
    } else if turn == 180.0 {
        (0.0, -1.0)
    } else if turn == 270.0 {
        (-1.0, 0.0)
    } else {
        turn.to_radians().sin_cos()
    }
}

/// `mm` on the 1 nm grid.
pub fn snap(mm: f64) -> f64 {
    (mm * 1e6).round() / 1e6
}

/// A net class with PCB rules (mm) and member nets.
pub fn class(name: &str, nets: &[&str], clearance: f64, trace: f64) -> NetClass {
    NetClass {
        nets: nets.iter().map(|n| n.to_string()).collect(),
        clearance,
        trace_width: trace,
        ..NetClass::named(name)
    }
}

/// A closed rectangle `(x0, y0)`..`(x1, y1)`.
pub fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
    vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)]
}

/// A rectangle with corners rounded to `r`, each corner tessellated with
/// `steps` segments (counter-clockwise in the model's axes).
pub fn rounded_rect(x0: f64, y0: f64, x1: f64, y1: f64, r: f64, steps: usize) -> Vec<Point> {
    let corners = [
        (x1 - r, y0 + r, -90.0),
        (x1 - r, y1 - r, 0.0),
        (x0 + r, y1 - r, 90.0),
        (x0 + r, y0 + r, 180.0),
    ];
    let mut out = Vec::new();
    for (cx, cy, start) in corners {
        out.extend(arc_points(p(cx, cy), r, start, 90.0, steps));
    }
    out
}

/// Points along an arc from `start_deg`, sweeping `sweep_deg`, both ends
/// included; each snapped to 1 nm.
pub fn arc_points(c: Point, r: f64, start_deg: f64, sweep_deg: f64, steps: usize) -> Vec<Point> {
    (0..=steps)
        .map(|i| {
            let t = (start_deg + sweep_deg * i as f64 / steps as f64).to_radians();
            p(snap(c.x + r * t.cos()), snap(c.y + r * t.sin()))
        })
        .collect()
}
