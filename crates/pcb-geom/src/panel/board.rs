//! The whole panel as one board (#2410), what the fabrication outputs are
//! made from: every copy of the board's items moved to its place, the frame
//! as the outline, the routed edges on the profile layer, the fiducials as
//! fixed parts, and the tooling holes, perforations and V-scores as the
//! board sketch's holes and score lines. Built for one output run and never
//! stored. Each copy's nets are its own (`GND/2` is copy 2's GND), so the
//! test netlist does not join the copies.

use signex_types::pcb::{
    BoardGraphic, ComponentAttributes, ComponentType, Footprint, NetDef,
    PCB_DEFAULT_GRAPHIC_WIDTH_MM, Pad, PadShape, PadType, PcbBoard, Point,
};
use signex_types::pcb_panel::{BoardPanel, PanelError};
use signex_types::pcb_sketch::{BoardSketch, PcbHole, PcbVScore};
use uuid::Uuid;

use super::layout::{PanelLayout, PanelSegment, layout};
use crate::rect::RectNm;
use crate::units::{PointNm, nm_to_mm};

/// The layer the routed edges are drawn on: the board profile.
pub const PANEL_ROUTE_LAYER: &str = "EdgeCuts";

const TOP_COPPER: &str = "TopCopper";
const TOP_SOLDER_MASK: &str = "TopSolderMask";

/// How deep a panel's V-score cuts, as a fraction of the thickness.
const V_SCORE_DEPTH: f64 = 1.0 / 3.0;

/// The uuids of the panel's own items: "PANEL" in the top bytes, the kind
/// of item, then its index.
const PANEL_UUID: u128 = 0x5041_4e45_4c00_0000_0000_0000_0000_0000;
const FIDUCIAL: u128 = 1;
const FIDUCIAL_PAD: u128 = 2;
const HOLE: u128 = 3;
const V_SCORE: u128 = 4;
const ROUTE: u128 = 5;

/// `board` arrayed into `panel`.
pub fn panel_board(board: &PcbBoard, panel: &BoardPanel) -> Result<PcbBoard, PanelError> {
    let layout = layout(&board.outline, panel)?;
    let highest = board.nets.iter().map(|n| n.number).max().unwrap_or(0);
    let stride = u64::from(highest) + 1;
    let copies = layout.copies.len() as u64;
    let stride = u32::try_from(stride)
        .ok()
        .filter(|_| stride * copies <= u64::from(u32::MAX))
        .ok_or(PanelError::TooLarge)?;
    let mut out = board.clone();
    out.options.panel = None;
    out.outline = corners(layout.frame);
    out.nets.clear();
    out.footprints.clear();
    out.segments.clear();
    out.vias.clear();
    out.zones.clear();
    out.graphics.clear();
    out.texts.clear();
    out.free = Default::default();
    let mut sketch = BoardSketch::default();
    for (index, offset) in (0u32..).zip(&layout.copies) {
        let copy = PlacedCopy {
            index,
            offset: offset.to_mm(),
            stride,
        };
        copy.add(board, &mut out, &mut sketch);
    }
    add_marks(&layout, &mut out, &mut sketch);
    out.sketch = Some(sketch);
    Ok(out)
}

fn corners(frame: RectNm) -> Vec<Point> {
    let (lo, hi) = (frame.min, frame.max);
    [lo, PointNm::new(hi.x, lo.y), hi, PointNm::new(lo.x, hi.y)]
        .into_iter()
        .map(PointNm::to_mm)
        .collect()
}

/// One copy of the board: where it goes and how its nets are numbered.
struct PlacedCopy {
    index: u32,
    offset: Point,
    stride: u32,
}

impl PlacedCopy {
    fn at(&self, p: Point) -> Point {
        Point::new(p.x + self.offset.x, p.y + self.offset.y)
    }

    fn net(&self, net: u32) -> u32 {
        if net == 0 {
            0
        } else {
            net + self.index * self.stride
        }
    }

    fn net_name(&self, name: &str) -> String {
        format!("{name}/{}", self.index + 1)
    }

    /// The first copy keeps the board's uuids; the others flip bits in the
    /// top word, so a copy's items stay apart from each other.
    fn uuid(&self, id: Uuid) -> Uuid {
        Uuid::from_u128(id.as_u128() ^ (u128::from(self.index) << 96))
    }

    fn add(&self, board: &PcbBoard, out: &mut PcbBoard, sketch: &mut BoardSketch) {
        for net in &board.nets {
            if net.number == 0 && self.index > 0 {
                continue;
            }
            let name = match net.number {
                0 => net.name.clone(),
                _ => self.net_name(&net.name),
            };
            out.nets.push(NetDef {
                number: self.net(net.number),
                name,
            });
        }
        for footprint in &board.footprints {
            let mut footprint = footprint.clone();
            footprint.uuid = self.uuid(footprint.uuid);
            footprint.position = self.at(footprint.position);
            for pad in &mut footprint.pads {
                pad.uuid = self.uuid(pad.uuid);
                if let Some(net) = pad.net.as_mut()
                    && net.number != 0
                {
                    net.number = self.net(net.number);
                    net.name = self.net_name(&net.name);
                }
            }
            out.footprints.push(footprint);
        }
        for segment in &board.segments {
            let mut segment = segment.clone();
            segment.uuid = self.uuid(segment.uuid);
            segment.start = self.at(segment.start);
            segment.end = self.at(segment.end);
            segment.net = self.net(segment.net);
            out.segments.push(segment);
        }
        for via in &board.vias {
            let mut via = via.clone();
            via.uuid = self.uuid(via.uuid);
            via.position = self.at(via.position);
            via.net = self.net(via.net);
            out.vias.push(via);
        }
        for zone in &board.zones {
            let mut zone = zone.clone();
            zone.uuid = self.uuid(zone.uuid);
            if zone.net != 0 {
                zone.net_name = self.net_name(&zone.net_name);
            }
            zone.net = self.net(zone.net);
            self.move_all(&mut zone.outline);
            for hole in &mut zone.holes {
                self.move_all(hole);
            }
            out.zones.push(zone);
        }
        for graphic in &board.graphics {
            let mut graphic = graphic.clone();
            graphic.uuid = self.uuid(graphic.uuid);
            for point in [&mut graphic.start, &mut graphic.end, &mut graphic.center] {
                *point = (*point).map(|p| self.at(p));
            }
            self.move_all(&mut graphic.points);
            out.graphics.push(graphic);
        }
        for text in &board.texts {
            let mut text = text.clone();
            text.uuid = self.uuid(text.uuid);
            text.position = self.at(text.position);
            out.texts.push(text);
        }
        // #2373: free arcs, pads and dimensions are copied with the rest.
        for arc in &board.free.arcs {
            let mut arc = arc.clone();
            arc.uuid = self.uuid(arc.uuid);
            arc.start = self.at(arc.start);
            arc.mid = self.at(arc.mid);
            arc.end = self.at(arc.end);
            arc.net = self.net(arc.net);
            out.free.arcs.push(arc);
        }
        for pad in &board.free.pads {
            let mut pad = pad.clone();
            pad.uuid = self.uuid(pad.uuid);
            pad.position = self.at(pad.position);
            if let Some(net) = pad.net.as_mut()
                && net.number != 0
            {
                net.number = self.net(net.number);
                net.name = self.net_name(&net.name);
            }
            out.free.pads.push(pad);
        }
        for dimension in &board.free.dimensions {
            let mut dimension = dimension.clone();
            dimension.uuid = self.uuid(dimension.uuid);
            dimension.start = self.at(dimension.start);
            dimension.end = self.at(dimension.end);
            out.free.dimensions.push(dimension);
        }
        if let Some(source) = &board.sketch {
            self.add_sketch(source, sketch);
        }
    }

    fn move_all(&self, points: &mut [Point]) {
        for point in points {
            *point = self.at(*point);
        }
    }

    fn add_sketch(&self, source: &BoardSketch, sketch: &mut BoardSketch) {
        for keepout in &source.keepouts {
            let mut keepout = keepout.clone();
            keepout.uuid = self.uuid(keepout.uuid);
            self.move_all(&mut keepout.boundary);
            sketch.keepouts.push(keepout);
        }
        for cutout in &source.cutouts {
            let mut cutout = cutout.clone();
            cutout.uuid = self.uuid(cutout.uuid);
            self.move_all(&mut cutout.boundary);
            sketch.cutouts.push(cutout);
        }
        for hole in &source.holes {
            let mut hole = hole.clone();
            hole.uuid = self.uuid(hole.uuid);
            hole.position = self.at(hole.position);
            hole.net = hole.net.map(|net| self.net(net));
            sketch.holes.push(hole);
        }
        for score in &source.v_scores {
            let mut score = score.clone();
            score.uuid = self.uuid(score.uuid);
            score.from = self.at(score.from);
            score.to = self.at(score.to);
            sketch.v_scores.push(score);
        }
    }
}

fn panel_uuid(kind: u128, index: usize) -> Uuid {
    Uuid::from_u128(PANEL_UUID | (kind << 64) | index as u128)
}

/// The fiducials, holes, score lines and routed edges of the panel itself.
fn add_marks(layout: &PanelLayout, out: &mut PcbBoard, sketch: &mut BoardSketch) {
    let dot = nm_to_mm(layout.fiducial_diameter);
    for (index, at) in layout.fiducials.iter().enumerate() {
        out.footprints.push(fiducial(index, at.to_mm(), dot));
    }
    let tooling = layout
        .tooling_holes
        .iter()
        .map(|at| (*at, layout.tooling_hole_diameter));
    let bites = layout
        .bite_holes
        .iter()
        .map(|at| (*at, layout.bite_diameter));
    for (index, (at, drill)) in tooling.chain(bites).enumerate() {
        sketch.holes.push(PcbHole {
            uuid: panel_uuid(HOLE, index),
            position: at.to_mm(),
            drill: nm_to_mm(drill),
            plated: false,
            net: None,
            unknown_columns: Default::default(),
        });
    }
    for (index, line) in layout.v_scores.iter().enumerate() {
        sketch.v_scores.push(PcbVScore {
            uuid: panel_uuid(V_SCORE, index),
            from: line.from.to_mm(),
            to: line.to.to_mm(),
            depth_fraction: V_SCORE_DEPTH,
            min_web: 0.0,
            side: Default::default(),
        });
    }
    out.graphics.extend(
        layout
            .routes
            .iter()
            .enumerate()
            .map(|(index, line)| route(index, line)),
    );
}

/// A fixed top-side copper dot that opens the mask over itself; graphical,
/// so no BOM or pick-and-place row lists it.
fn fiducial(index: usize, at: Point, diameter: f64) -> Footprint {
    let pad = Pad {
        uuid: panel_uuid(FIDUCIAL_PAD, index),
        number: "1".to_string(),
        pad_type: PadType::Smd,
        shape: PadShape::Circle,
        position: Point::new(0.0, 0.0),
        size: Point::new(diameter, diameter),
        drill: None,
        // Listing the mask opening keeps the fiducial off the paste image.
        layers: vec![TOP_COPPER.to_string(), TOP_SOLDER_MASK.to_string()],
        net: None,
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
        uuid: panel_uuid(FIDUCIAL, index),
        reference: format!("FID{}", index + 1),
        value: "Fiducial".to_string(),
        footprint_id: "Fiducial".to_string(),
        position: at,
        rotation: 0.0,
        layer: TOP_COPPER.to_string(),
        locked: true,
        pads: vec![pad],
        graphics: Vec::new(),
        properties: Vec::new(),
        schematic_link: None,
        library_ref: None,
        component: Box::new(ComponentAttributes {
            component_type: ComponentType::Graphical,
            ..ComponentAttributes::default()
        }),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn route(index: usize, line: &PanelSegment) -> BoardGraphic {
    BoardGraphic {
        uuid: panel_uuid(ROUTE, index),
        graphic_type: "line".to_string(),
        layer: PANEL_ROUTE_LAYER.to_string(),
        width: PCB_DEFAULT_GRAPHIC_WIDTH_MM,
        start: Some(line.from.to_mm()),
        end: Some(line.to.to_mm()),
        center: None,
        radius: 0.0,
        points: Vec::new(),
        text: String::new(),
        font_size: 0.0,
    }
}
