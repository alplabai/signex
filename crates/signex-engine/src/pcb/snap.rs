//! Object, guide and axis snapping on a board (#2353, #2374): the point of a
//! board object, or of a guide, the pointer is pulled to while footprints
//! are carried. Pure: it reads the board and changes nothing.

use std::collections::BTreeSet;

use pcb_geom::PickKind;
use signex_types::pcb::{PcbBoard, Point};
use signex_types::pcb_layer::PcbLayer;
use signex_types::pcb_options::{BoardGuide, GuideKind};
use uuid::Uuid;

use super::snap_points::candidates;

/// One kind of point the pointer can snap to, in the order the Snap Options
/// list shows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SnapObject {
    TrackVertices,
    TrackLines,
    ArcCenters,
    Intersections,
    PadCenters,
    PadVertices,
    PadEdges,
    ViaCenters,
    Regions,
    BoardShape,
    FootprintOrigins,
    Body3dPoints,
    Texts,
}

impl SnapObject {
    pub const ALL: [SnapObject; 13] = [
        SnapObject::TrackVertices,
        SnapObject::TrackLines,
        SnapObject::ArcCenters,
        SnapObject::Intersections,
        SnapObject::PadCenters,
        SnapObject::PadVertices,
        SnapObject::PadEdges,
        SnapObject::ViaCenters,
        SnapObject::Regions,
        SnapObject::BoardShape,
        SnapObject::FootprintOrigins,
        SnapObject::Body3dPoints,
        SnapObject::Texts,
    ];

    /// The kinds snapped to on a new session.
    pub const DEFAULT_ON: [SnapObject; 2] = [SnapObject::PadCenters, SnapObject::ViaCenters];

    /// Whether [`object_snap_point`] finds points of this kind. A board
    /// holds no 3D bodies yet, so it has no body points.
    pub fn is_supported(self) -> bool {
        self != SnapObject::Body3dPoints
    }
}

/// What object snapping looks for.
#[derive(Debug, Clone, Copy)]
pub struct SnapQuery<'a> {
    pub objects: &'a BTreeSet<SnapObject>,
    /// The copper layers whose objects count, as `pcb_geom::layer_bit`
    /// bits; `u32::MAX` for every layer. Texts and graphics count only on
    /// every layer, the board shape on any.
    pub layers: u32,
    /// Footprints whose pads and origins are left out, and vias, tracks and
    /// arcs whose points are: the carried ones and those following them.
    pub skip: &'a BTreeSet<Uuid>,
    /// How far from the pointer a point may be, mm.
    pub radius_mm: f64,
    /// The layers the view shows: a hidden layer gives no points.
    pub shown: ShownLayers<'a>,
    /// While a route runs, its net: copper of another net gives no points.
    /// Copper of no net (0) and objects without a net still do.
    pub net: Option<u32>,
}

/// What the board view shows, as object snapping reads it.
#[derive(Debug, Clone, Copy)]
pub struct ShownLayers<'a> {
    /// The copper layers shown, as `pcb_geom::layer_bit` bits.
    pub copper: u32,
    /// The other layers it hides.
    pub hidden: &'a [PcbLayer],
    /// The Multi-Layer row is off: no vias or through-hole pads.
    pub through_hidden: bool,
    /// The kinds of object the view hides.
    pub hidden_kinds: &'a [PickKind],
}

impl ShownLayers<'_> {
    /// Every layer and kind shown.
    pub const ALL: ShownLayers<'static> = ShownLayers {
        copper: u32::MAX,
        hidden: &[],
        through_hidden: false,
        hidden_kinds: &[],
    };

    /// Whether objects of `kind` are shown.
    pub(super) fn shows(&self, kind: PickKind) -> bool {
        !self.hidden_kinds.contains(&kind)
    }
}

fn usable(radius: f64) -> bool {
    radius.is_finite() && radius > 0.0
}

fn distance_sq(a: Point, b: Point) -> f64 {
    (a.x - b.x).powi(2) + (a.y - b.y).powi(2)
}

/// The snap point nearest `at` within the query's radius, if any.
pub fn object_snap_point(board: &PcbBoard, at: Point, query: &SnapQuery) -> Option<Point> {
    if !usable(query.radius_mm) {
        return None;
    }
    let reach = query.radius_mm * query.radius_mm;
    candidates(board, query, at)
        .into_iter()
        .filter(|p| p.x.is_finite() && p.y.is_finite())
        .map(|p| (distance_sq(p, at), p))
        .filter(|(d, _)| *d <= reach)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, p)| p)
}

/// The x and the y axis snapping lines `at` up with (#2374): of the snap
/// points within `range_mm` of the pointer, the nearest one whose x is
/// within the query's radius of the pointer's x gives the x, and the same
/// for y. `None` on an axis nothing lines up on.
pub fn axis_snap(
    board: &PcbBoard,
    at: Point,
    query: &SnapQuery,
    range_mm: f64,
) -> (Option<f64>, Option<f64>) {
    if !usable(query.radius_mm) || !usable(range_mm) {
        return (None, None);
    }
    let reach = range_mm * range_mm;
    let near: Vec<(f64, Point)> = candidates(board, query, at)
        .into_iter()
        .filter(|p| p.x.is_finite() && p.y.is_finite())
        .map(|p| (distance_sq(p, at), p))
        .filter(|(d, _)| *d <= reach)
        .collect();
    let along = |gap: fn(Point, Point) -> f64, value: fn(Point) -> f64| {
        near.iter()
            .filter(|(_, p)| gap(*p, at) <= query.radius_mm)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, p)| value(*p))
    };
    (
        along(|p, q| (p.x - q.x).abs(), |p| p.x),
        along(|p, q| (p.y - q.y).abs(), |p| p.y),
    )
}

/// What a guide pulls the pointer to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GuideHit {
    /// Onto one guide line at `point`; `kind` says which axes it fixes.
    Line { kind: GuideKind, point: Point },
    /// Onto the crossing of two guides.
    Crossing(Point),
}

/// Where the enabled `guides` pull `at` within `radius_mm` (#2374): a
/// crossing of two guides first, else the nearest guide line.
pub fn guide_snap(guides: &[BoardGuide], at: Point, radius_mm: f64) -> Option<GuideHit> {
    if !usable(radius_mm) {
        return None;
    }
    let reach = radius_mm * radius_mm;
    let live: Vec<&BoardGuide> = guides
        .iter()
        .filter(|g| g.enabled && g.at.x.is_finite() && g.at.y.is_finite())
        .collect();
    let crossing = live
        .iter()
        .enumerate()
        .flat_map(|(i, a)| live[i + 1..].iter().filter_map(move |b| a.crossing(b)))
        .map(|p| (distance_sq(p, at), p))
        .filter(|(d, _)| *d <= reach)
        .min_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((_, p)) = crossing {
        return Some(GuideHit::Crossing(p));
    }
    live.iter()
        .map(|g| (g.kind, g.nearest_point(at)))
        .map(|(kind, p)| (distance_sq(p, at), kind, p))
        .filter(|(d, _, _)| *d <= reach)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, kind, point)| GuideHit::Line { kind, point })
}

#[cfg(test)]
#[path = "snap_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "snap_more_tests.rs"]
mod more_tests;
