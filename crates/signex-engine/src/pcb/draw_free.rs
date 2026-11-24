//! The free arcs, pads and dimensions the board placement tools draw
//! (#2373), re-exported from `draw`. Pure: each builds the item one click
//! (or the last click of a shape) adds, with the board's defaults and the
//! net of the copper it lands on.

use std::f64::consts::TAU;

use pcb_geom::layer_bit;
use signex_types::format::pcb_layer_token;
use signex_types::pcb::{DrillDef, Pad, PadNet, PadShape, PadType, PcbBoard, Point};
use signex_types::pcb_arc::{ArcSweep, TrackArc};
use signex_types::pcb_dimension::LinearDimension;
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
use uuid::Uuid;

use super::draw::net_under;

/// The size of a new free pad, mm: round, 1.5 mm across.
pub const FREE_PAD_SIZE_MM: f64 = 1.5;
/// The hole of a new free pad, mm.
pub const FREE_PAD_HOLE_MM: f64 = 0.8;

/// How the Arc tool's clicks make an arc.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ArcMode {
    /// The centre, then the start (which sets the radius), then a point
    /// whose direction from the centre ends the arc. The arc runs
    /// counter-clockwise on screen from start to end.
    #[default]
    Centre,
    /// The two ends of a quarter circle.
    Edge,
    /// The start, the end, then any point on the arc, which picks its bend.
    AnyAngle,
}

impl ArcMode {
    /// Every mode, in the order the tool steps through them.
    pub const ALL: [Self; 3] = [Self::Centre, Self::Edge, Self::AnyAngle];

    /// How many clicks place one arc.
    pub fn clicks(self) -> usize {
        match self {
            Self::Edge => 2,
            Self::Centre | Self::AnyAngle => 3,
        }
    }

    /// The next mode in [`Self::ALL`].
    pub fn next(self) -> Self {
        let at = Self::ALL.iter().position(|m| *m == self).unwrap_or(0);
        Self::ALL[(at + 1) % Self::ALL.len()]
    }
}

/// The start, a point on the arc and the end of the arc `mode` makes of
/// `clicks` (as many as [`ArcMode::clicks`]). `None` while there are too
/// few, or when they make no arc (a zero radius, points on one line, the
/// same point twice).
pub fn arc_points(mode: ArcMode, clicks: &[Point]) -> Option<(Point, Point, Point)> {
    let points = match (mode, clicks) {
        (ArcMode::Centre, &[centre, start, toward, ..]) => by_centre(centre, start, toward)?,
        (ArcMode::Edge, &[start, end, ..]) => by_edge(start, end)?,
        (ArcMode::AnyAngle, &[start, end, on, ..]) => (start, on, end),
        _ => return None,
    };
    let probe = TrackArc {
        uuid: Uuid::nil(),
        start: points.0,
        mid: points.1,
        end: points.2,
        width: 0.0,
        layer: String::new(),
        net: 0,
    };
    probe.sweep().map(|_| points)
}

/// Model y grows down, so counter-clockwise on screen is a falling angle.
fn by_centre(centre: Point, start: Point, toward: Point) -> Option<(Point, Point, Point)> {
    let radius = (start.x - centre.x).hypot(start.y - centre.y);
    if !(radius.is_finite() && radius > 0.0) || toward == centre {
        return None;
    }
    let start_angle = (start.y - centre.y).atan2(start.x - centre.x);
    let end_angle = (toward.y - centre.y).atan2(toward.x - centre.x);
    let turn = (start_angle - end_angle).rem_euclid(TAU);
    if turn == 0.0 {
        return None;
    }
    Some(
        ArcSweep {
            center: centre,
            radius,
            start_angle,
            sweep: -turn,
        }
        .three_points(),
    )
}

/// The start, middle and end of the arc about `centre` from `start` (its
/// distance is the radius) to the angle of `toward`, the way of increasing
/// angle in the model's axes (clockwise on screen): the board Graphics Arc
/// drawn on copper, as a track arc (#2869). `None` for a point on the
/// centre, or a full turn, which no track arc is.
pub fn arc_points_about(
    centre: Point,
    start: Point,
    toward: Point,
) -> Option<(Point, Point, Point)> {
    let radius = (start.x - centre.x).hypot(start.y - centre.y);
    if !(radius.is_finite() && radius > 0.0) || toward == centre {
        return None;
    }
    let start_angle = (start.y - centre.y).atan2(start.x - centre.x);
    let end_angle = (toward.y - centre.y).atan2(toward.x - centre.x);
    let turn = (end_angle - start_angle).rem_euclid(TAU);
    if turn == 0.0 {
        return None;
    }
    Some(
        ArcSweep {
            center: centre,
            radius,
            start_angle,
            sweep: turn,
        }
        .three_points(),
    )
}

fn by_edge(start: Point, end: Point) -> Option<(Point, Point, Point)> {
    let half = Point::new((end.x - start.x) / 2.0, (end.y - start.y) / 2.0);
    if half.x == 0.0 && half.y == 0.0 {
        return None;
    }
    // The centre stands half the chord off its middle, a quarter turn from
    // it: the chord then subtends a quarter turn. The arc's middle is the
    // radius from the centre, through the chord's middle.
    let centre = Point::new(start.x + half.x - half.y, start.y + half.y + half.x);
    let reach = std::f64::consts::SQRT_2;
    let mid = Point::new(centre.x + half.y * reach, centre.y - half.x * reach);
    Some((start, mid, end))
}

/// An arc through `points` (start, a point on it, end) on `layer`, `width`
/// wide. On copper it takes the net of the copper under its start; off
/// copper it has none. `None` for points that make no arc.
pub fn new_arc(
    board: &PcbBoard,
    id: Uuid,
    points: (Point, Point, Point),
    layer: PcbLayer,
    width_mm: f64,
) -> Option<TrackArc> {
    let net = match layer.copper() {
        Some(copper) => net_under(board, points.0, 1 << layer_bit(copper)),
        None => 0,
    };
    let arc = TrackArc {
        uuid: id,
        start: points.0,
        mid: points.1,
        end: points.2,
        width: width_mm,
        layer: pcb_layer_token(layer),
        net,
    };
    arc.sweep().map(|_| arc)
}

/// A free through-hole pad at `at`: round, [`FREE_PAD_SIZE_MM`] across
/// with a [`FREE_PAD_HOLE_MM`] hole, on every copper layer, numbered
/// `number`, on the net of the copper it lands on.
pub fn new_free_pad(board: &PcbBoard, id: Uuid, at: Point, number: String) -> Pad {
    let net = net_under(board, at, u32::MAX);
    let net = (net != 0).then(|| pad_net(board, net));
    Pad {
        uuid: id,
        number,
        pad_type: PadType::Thru,
        shape: PadShape::Circle,
        position: at,
        size: Point::new(FREE_PAD_SIZE_MM, FREE_PAD_SIZE_MM),
        drill: Some(DrillDef {
            diameter: FREE_PAD_HOLE_MM,
            shape: String::new(),
        }),
        layers: vec![
            pcb_layer_token(PcbLayer::Copper(CopperLayer::Top)),
            pcb_layer_token(PcbLayer::Copper(CopperLayer::Bottom)),
        ],
        net,
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

/// The free pad the Pad tool places at `at` with `layer` active: on the top
/// or bottom copper a round surface pad of that side (its copper, solder
/// mask and paste, no hole) on the net of that copper under it; on any
/// other layer the through-hole pad of [`new_free_pad`]. An inner layer
/// keeps the through-hole pad: a surface pad there has no mask or paste.
pub fn new_free_pad_on(
    board: &PcbBoard,
    id: Uuid,
    at: Point,
    number: String,
    layer: PcbLayer,
) -> Pad {
    let (copper, mask, paste) = match layer {
        PcbLayer::Copper(CopperLayer::Top) => (
            CopperLayer::Top,
            TechnicalLayer::SolderMaskTop,
            TechnicalLayer::PasteTop,
        ),
        PcbLayer::Copper(CopperLayer::Bottom) => (
            CopperLayer::Bottom,
            TechnicalLayer::SolderMaskBottom,
            TechnicalLayer::PasteBottom,
        ),
        _ => return new_free_pad(board, id, at, number),
    };
    let mut pad = new_free_pad(board, id, at, number);
    let net = net_under(board, at, 1 << layer_bit(copper));
    pad.net = (net != 0).then(|| pad_net(board, net));
    pad.pad_type = PadType::Smd;
    pad.drill = None;
    pad.layers = [
        PcbLayer::Copper(copper),
        PcbLayer::Technical(mask),
        PcbLayer::Technical(paste),
    ]
    .into_iter()
    .map(pcb_layer_token)
    .collect();
    pad
}

/// Net `number` of `board` as a pad holds it.
fn pad_net(board: &PcbBoard, number: u32) -> PadNet {
    PadNet {
        number,
        name: board
            .nets
            .iter()
            .find(|n| n.number == number)
            .map(|n| n.name.clone())
            .unwrap_or_default(),
    }
}

/// A linear dimension from `start` to `end`, its line through `through`
/// (the offset is how far `through` stands off the measured line), on
/// `layer`, whichever layer it is, copper included (Caner, 2026-11-21).
/// `None` when `start` and `end` are the same point.
pub fn new_dimension(
    id: Uuid,
    start: Point,
    end: Point,
    through: Point,
    layer: PcbLayer,
) -> Option<LinearDimension> {
    let (dx, dy) = (end.x - start.x, end.y - start.y);
    let length = dx.hypot(dy);
    if !(length.is_finite() && length > 0.0) {
        return None;
    }
    // The offset runs along the quarter turn from start to end, as
    // `LinearDimension::offset` reads it.
    let offset = ((through.x - start.x) * -dy + (through.y - start.y) * dx) / length;
    let mut dimension = LinearDimension::new(id, start, end, offset);
    dimension.layer = pcb_layer_token(layer);
    Some(dimension)
}
