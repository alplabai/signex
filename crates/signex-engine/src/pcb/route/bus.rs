//! Several nets routed side by side (#2389): a differential pair, or a bus
//! of selected nets (Multi-Route). The route is one centre line; each
//! member follows it at its offset (`pcb_geom::route::parallel_paths`),
//! leftmost member on the left, joined straight to its own start point.
//! The centre line walks around other nets' copper as wide as the whole
//! bus, its own members' copper no obstacle (`RouteWorld::with_kin`); the
//! member tracks are then checked one by one, gaps between members
//! included, by [`super::draft_fits`].

use pcb_geom::route::{RouteWorld, Walk, bus_offsets, parallel_paths, walkaround};
use pcb_geom::{PointNm, Shape, collect_copper, layer_bit, mm_to_nm};
use signex_types::format::pcb_layer_token;
use signex_types::pcb::{PcbBoard, Point, Segment};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use uuid::Uuid;

use super::{RouteRefusal, RouteRules, lattice_step, nm};

/// The tracks of a bus: their layer, width and the distance between the
/// centre lines of two neighbours.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BusStyle {
    pub layer: CopperLayer,
    pub width_mm: f64,
    pub pitch_mm: f64,
}

/// One net of a bus and where its tracks start.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BusMember {
    pub net: u32,
    pub anchor: Point,
}

/// The point of `net`'s copper on `layer` nearest `at`: a pad centre, a
/// track end or a via. `None` when the net has no copper there.
pub fn nearest_net_point(
    board: &PcbBoard,
    net: u32,
    layer: CopperLayer,
    at: Point,
) -> Option<Point> {
    let at = PointNm::from_mm(at, "point").ok()?;
    let on_layer = 1 << layer_bit(layer);
    collect_copper(board)
        .into_iter()
        .filter(|item| item.net == net && item.layers & on_layer != 0)
        .flat_map(|item| match (item.pad_center, item.shape) {
            (Some(center), _) => vec![center],
            (None, Shape::Capsule { a, b, .. }) => vec![a, b],
            (None, Shape::Disc { center, .. }) => vec![center],
            (None, _) => Vec::new(),
        })
        .min_by_key(|p| p.distance_sq(at))
        .map(PointNm::to_mm)
}

/// Where the centre line of `members` starts: the middle of their start
/// points. `None` for no member.
pub fn bus_start(members: &[BusMember]) -> Option<Point> {
    if members.is_empty() {
        return None;
    }
    let n = members.len() as f64;
    let (x, y) = members
        .iter()
        .fold((0.0, 0.0), |(x, y), m| (x + m.anchor.x, y + m.anchor.y));
    Some(Point::new(x / n, y / n))
}

/// The centre line of `members` from `from` to `to` around other nets'
/// copper, wide enough for every member: its corners after `from`, `to`
/// included.
pub fn route_bus_leg(
    board: &PcbBoard,
    members: &[BusMember],
    from: Point,
    to: Point,
    style: &BusStyle,
    rules: &RouteRules,
) -> Result<Vec<Point>, RouteRefusal> {
    let first = members.first().ok_or(RouteRefusal::Blocked)?;
    let nets: Vec<u32> = members.iter().map(|m| m.net).collect();
    let (a, b) = (nm(from)?, nm(to)?);
    let (width, pitch) = widths(style)?;
    let widest = bus_offsets(members.len(), pitch)
        .into_iter()
        .map(i64::abs)
        .max()
        .unwrap_or(0);
    let world = RouteWorld::with_reach(board, rules.clearance, rules.reach).with_kin(&nets);
    // The outer member's mitred corner reaches √2 offsets from a right-angle
    // bend: the centre line keeps room for one and a half.
    let walk = Walk {
        net: first.net,
        layers: 1 << layer_bit(style.layer),
        radius: widest * 3 / 2 + width / 2,
        rule: rules.corner,
        step: lattice_step(rules.grid_mm, a, b),
        budget: rules.budget,
    };
    let path = walkaround(&world, a, b, &walk).ok_or(RouteRefusal::Blocked)?;
    Ok(path
        .into_iter()
        .skip(1)
        .map(|p| if p == b { to } else { p.to_mm() })
        .collect())
}

/// The members' tracks along `centre`: each member at its offset, joined
/// straight from its start point. Empty for a centre line with no leg yet.
pub fn bus_tracks(
    members: &[BusMember],
    centre: &[Point],
    style: &BusStyle,
    mut ids: impl FnMut() -> Uuid,
) -> Result<Vec<Segment>, RouteRefusal> {
    let centre: Vec<PointNm> = centre.iter().map(|&p| nm(p)).collect::<Result<_, _>>()?;
    let Some(heading) = centre
        .iter()
        .find(|&&p| p != centre[0])
        .map(|p| p.minus(centre[0]))
    else {
        return Ok(Vec::new());
    };
    let (_, pitch) = widths(style)?;
    let mut ordered: Vec<(BusMember, PointNm)> = members
        .iter()
        .map(|m| nm(m.anchor).map(|anchor| (*m, anchor)))
        .collect::<Result<_, _>>()?;
    // Leftmost first: the member whose start lies farthest to the left of
    // the first leg takes the leftmost offset.
    ordered.sort_by_key(|(_, anchor)| std::cmp::Reverse(heading.cross(anchor.minus(centre[0]))));
    let offsets = bus_offsets(ordered.len(), pitch);
    let paths = parallel_paths(&centre, &offsets).ok_or(RouteRefusal::TooSharp)?;
    let layer = pcb_layer_token(PcbLayer::Copper(style.layer));
    let mut tracks = Vec::new();
    for ((member, anchor), path) in ordered.into_iter().zip(paths) {
        let mut points = vec![anchor];
        points.extend(path);
        points.dedup();
        let point = |p: PointNm| {
            if p == anchor {
                member.anchor
            } else {
                p.to_mm()
            }
        };
        tracks.extend(points.windows(2).map(|leg| Segment {
            uuid: ids(),
            locked: false,
            attributes: Default::default(),
            start: point(leg[0]),
            end: point(leg[1]),
            width: style.width_mm,
            layer: layer.clone(),
            net: member.net,
            unknown_columns: Default::default(),
        }));
    }
    Ok(tracks)
}

/// What rounding the members' corners to whole nm can take off the gap
/// between two neighbours, added to the pitch: a gap set exactly at the
/// rule clearance stays legal on a diagonal leg.
const ROUNDING_NM: i64 = 4;

/// The width and pitch in nm, the pitch with [`ROUNDING_NM`] added;
/// refused when one is not a length the geometry can hold or the pitch
/// leaves no room for the width.
fn widths(style: &BusStyle) -> Result<(i64, i64), RouteRefusal> {
    let width = mm_to_nm(style.width_mm, "track width")
        .map_err(|_| RouteRefusal::Unrepresentable)?
        .abs();
    let pitch = mm_to_nm(style.pitch_mm, "bus pitch")
        .map_err(|_| RouteRefusal::Unrepresentable)?
        .abs();
    if pitch <= width {
        return Err(RouteRefusal::Unrepresentable);
    }
    Ok((width, pitch + ROUNDING_NM))
}
