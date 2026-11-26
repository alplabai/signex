//! Thermal relief spokes (#2580, #2593): a pour with Thermal Relief on keeps
//! each pad of its net on its layer clear by an air gap and joins it through
//! spokes, the pad's own values winning over the pour's
//! (`Zone::pour_relief`). Rules: `docs/PCB_TOOLS.md`, "Thermal relief".

use signex_types::pad_thermal::{PadThermal, Relief};
use signex_types::pcb::{Footprint, Pad, PadType, PcbBoard, Zone};
use signex_types::pcb_free::free_pad_frame;

use crate::copper::{pad_pieces, pieces_on};
use crate::rect::RectNm;
use crate::region::{Polygon, intersection, union};
use crate::transform::{pad_world_center, pad_world_rotation_deg, rotate, sin_cos_deg};
use crate::units::{PointNm, mm_to_nm};

/// Points per quarter turn of a round air gap ring.
const RING_ARC_STEPS: usize = 8;

/// What a pour's relief does round the pads of its net.
#[derive(Debug, Default)]
pub(super) struct Reliefs {
    /// Each relieved pad grown by its air gap: the fill keeps clear of it.
    pub clear: Vec<Polygon>,
    /// The spokes, each from its pad's centre across the gap and a spoke
    /// width into the fill.
    pub spokes: Vec<Polygon>,
}

/// The reliefs of `zone`'s pads on `layers` that can reach the fill; none
/// when the pour joins its pads solidly or has no net.
pub(super) fn reliefs(board: &PcbBoard, zone: &Zone, layers: u32, reach: RectNm) -> Reliefs {
    let mut out = Reliefs::default();
    if zone.net == 0 || zone.pour_relief(&PadThermal::RULE).is_none() {
        return out;
    }
    let frame = free_pad_frame();
    let pads = board
        .footprints
        .iter()
        .flat_map(|fp| fp.pads.iter().map(move |pad| (fp, pad)))
        .chain(board.free.pads.iter().map(|pad| (&frame, pad)));
    for (footprint, pad) in pads {
        let net = pad.net.as_ref().map_or(0, |n| n.number);
        if net != zone.net || pad.pad_type == PadType::NpThru {
            continue;
        }
        if let Some(relief) = zone.pour_relief(&pad.thermal) {
            relieve(footprint, pad, relief, layers, reach, &mut out);
        }
    }
    out
}

/// Add `pad`'s air gap rings and spokes on `layers`; a pad the geometry
/// cannot place adds nothing.
fn relieve(
    footprint: &Footprint,
    pad: &Pad,
    relief: Relief,
    layers: u32,
    reach: RectNm,
    out: &mut Reliefs,
) {
    let (Ok(gap), Ok(width), Ok(center), Ok(turn), Ok(pieces)) = (
        mm_to_nm(relief.air_gap, "thermal air gap"),
        mm_to_nm(relief.spoke_width, "thermal spoke width"),
        pad_world_center(footprint, pad),
        pad_world_rotation_deg(footprint, pad),
        pad_pieces(footprint, pad),
    ) else {
        return;
    };
    for shape in pieces_on(pieces, layers) {
        let ring = Polygon::new(
            shape.inflated(gap).covering_ring(RING_ARC_STEPS),
            Vec::new(),
        );
        if !ring.bounds().is_some_and(|b| b.intersects(&reach)) {
            continue;
        }
        let across = ring
            .outer
            .iter()
            .map(|p| (p.distance_sq(center) as f64).sqrt().ceil() as i64)
            .max()
            .unwrap_or(0);
        let length = across + width;
        for k in 0..relief.spokes {
            let angle = turn + 360.0 * f64::from(k) / f64::from(relief.spokes);
            out.spokes.push(spoke(center, length, width, angle));
        }
        out.clear.push(ring);
    }
}

/// A spoke `width` wide from `center` out to `length`, turned `angle_deg`
/// counter-clockwise from the board's x axis.
fn spoke(center: PointNm, length: i64, width: i64, angle_deg: f64) -> Polygon {
    let (sin, cos) = sin_cos_deg(angle_deg);
    let half = width / 2;
    let corners = [
        PointNm::new(0, -half),
        PointNm::new(length, -half),
        PointNm::new(length, half),
        PointNm::new(0, half),
    ];
    let ring = corners
        .iter()
        .map(|&c| center.plus(rotate(c, sin, cos)))
        .collect();
    Polygon::new(ring, Vec::new())
}

/// `fill` with `spokes` joined to it. A spoke that meets no fill (the
/// copper round its pad was cleared or thinned away) is left out, so no
/// stub stands alone in the gap.
pub(super) fn joined(fill: Vec<Polygon>, spokes: &[Polygon]) -> Vec<Polygon> {
    if spokes.is_empty() {
        return fill;
    }
    union(&fill, spokes)
        .into_iter()
        .filter(|piece| !intersection(std::slice::from_ref(piece), &fill).is_empty())
        .collect()
}

#[cfg(test)]
#[path = "relief_tests.rs"]
mod tests;
