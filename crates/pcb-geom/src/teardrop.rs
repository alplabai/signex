//! Teardrops (#2408): where a track enters a pad or via of its own net, a
//! tapered piece of copper from the track's width out to most of the pad's
//! width, so the joint keeps its copper when a drill wanders. One per track
//! end, straight-sided; skipped where it would come closer than the
//! clearance to copper of another net or meet a keepout that forbids
//! routing. The board's teardrop rule (`board.options.teardrops`) says
//! which pads and vias take one and how wide and long it is. Rules:
//! `docs/PCB_TOOLS.md`, "Teardrops" and "Teardrop rule".

use std::collections::BTreeSet;

use signex_types::pcb::{PcbBoard, Segment};
use signex_types::pcb_teardrop_rule::TeardropRule;
use uuid::Uuid;

use crate::copper::{layer_mask, pad_layers, via_layers};
use crate::index::track_shape;
use crate::obstacle::{NewCopper, copper_obstacles, keepout_areas, ring_clashes};
use crate::shape::Shape;
use crate::transform::{pad_shape, pad_world_center};
use crate::units::{PointNm, mm_to_nm};

/// One teardrop: its outline and what it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Teardrop {
    /// The track it widens.
    pub track: Uuid,
    pub net: u32,
    /// The track's layer token.
    pub layer: String,
    /// Four corners: the two at the pad, then the two at the track.
    pub outline: Vec<PointNm>,
}

/// What [`teardrops`] found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TeardropPlan {
    pub drops: Vec<Teardrop>,
    /// Track ends at a pad or via whose teardrop would break the clearance
    /// or a keepout, and so were left as they are.
    pub skipped: usize,
}

/// The teardrops of every track end inside a pad or via of its net, for the
/// nets in `nets` (`None`: every net). A track as wide as the pad, or too
/// short to hold a taper, gets none. `clearance(a, b)` is the gap nets `a`
/// and `b` need between them.
pub fn teardrops<F: Fn(u32, u32) -> i64>(
    board: &PcbBoard,
    nets: Option<&BTreeSet<u32>>,
    clearance: F,
) -> TeardropPlan {
    let rule = &board.options.teardrops;
    let obstacles = copper_obstacles(board);
    let keepouts = keepout_areas(board);
    let mut plan = TeardropPlan::default();
    for segment in &board.segments {
        let layers = layer_mask(std::iter::once(segment.layer.as_str()));
        let wanted = nets.is_none_or(|nets| nets.contains(&segment.net));
        if segment.net == 0 || layers == 0 || !wanted {
            continue;
        }
        let Ok(Shape::Capsule { a, b, radius: half }) = track_shape(segment) else {
            continue;
        };
        for (near, far) in [(a, b), (b, a)] {
            let Some((center, radius)) = anchor(board, rule, segment, layers, near) else {
                continue;
            };
            let Some(outline) = taper(rule, center, radius, near, far, half) else {
                continue;
            };
            let blocked = ring_clashes(&outline, segment.net, layers, &obstacles, &clearance)
                || keepouts
                    .iter()
                    .any(|k| k.bars_ring(NewCopper::Route, layers, &outline));
            if blocked {
                plan.skipped += 1;
                continue;
            }
            plan.drops.push(Teardrop {
                track: segment.uuid,
                net: segment.net,
                layer: segment.layer.clone(),
                outline,
            });
        }
    }
    plan
}

/// The centre and inscribed radius of the via or pad of the track's net
/// that holds `end` on the track's layer and takes a teardrop by `rule`;
/// vias first.
fn anchor(
    board: &PcbBoard,
    rule: &TeardropRule,
    track: &Segment,
    layers: u32,
    end: PointNm,
) -> Option<(PointNm, i64)> {
    let via = board.vias.iter().filter(|_| rule.vias).find_map(|via| {
        let center = PointNm::from_mm(via.position, "via.position").ok()?;
        let radius = (mm_to_nm(via.diameter, "via.diameter").ok()? / 2).abs();
        let holds = via.net == track.net
            && via_layers(via) & layers != 0
            && center.distance_sq(end) <= i128::from(radius) * i128::from(radius);
        holds.then_some((center, radius))
    });
    via.or_else(|| {
        board.footprints.iter().find_map(|footprint| {
            footprint.pads.iter().find_map(|pad| {
                let on_net = pad.net.as_ref().is_some_and(|n| n.number == track.net);
                if !on_net || !rule.pad_wants(pad.uuid) || pad_layers(footprint, pad) & layers == 0
                {
                    return None;
                }
                if !pad_shape(footprint, pad).ok()?.contains(end, 0) {
                    return None;
                }
                let center = pad_world_center(footprint, pad).ok()?;
                let side = pad.size.x.abs().min(pad.size.y.abs());
                let radius = mm_to_nm(side / 2.0, "pad.size").ok()?;
                Some((center, radius))
            })
        })
    })
}

/// The teardrop of the track from `near` (inside a pad of `radius` at
/// `center`) to `far`, whose half width is `half`. It runs along the track:
/// its base crosses the track's line at the point nearest the pad centre,
/// on both sides as far as the pad's nearer edge allows, so a track that
/// enters off centre or at an angle keeps both sides on it and the base
/// stays inside the pad. The rule's shares set how far across (of the
/// nearer edge) and how far along (in pad radii past the edge) it reaches.
/// `None` when the track is as wide as that base or too short to hold the
/// taper.
fn taper(
    rule: &TeardropRule,
    center: PointNm,
    radius: i64,
    near: PointNm,
    far: PointNm,
    half: i64,
) -> Option<Vec<PointNm>> {
    let (dx, dy) = ((far.x - near.x) as f64, (far.y - near.y) as f64);
    let length = dx.hypot(dy);
    if length == 0.0 {
        return None;
    }
    let (ux, uy) = (dx / length, dy / length);
    let (nx, ny) = (-uy, ux);
    let (cx, cy) = ((center.x - near.x) as f64, (center.y - near.y) as f64);
    // The base point on the track's line, and the pad centre's offset off it.
    let foot = cx * ux + cy * uy;
    let off = cx * nx + cy * ny;
    let (r, h) = (radius as f64, half as f64);
    if off.abs() >= r {
        return None;
    }
    // Where the track's line leaves the pad, counted from the foot.
    let chord = (r * r - off * off).sqrt();
    // The base runs across the track through the foot, on a line through
    // the pad centre, so the pad's nearer edge is `r - |off|` away.
    let base = (r - off.abs()) * rule.width_share;
    let reach = length - foot;
    if reach <= chord || base <= h * 1.05 {
        return None;
    }
    // The tip lies the rule's share of a pad radius past the pad's edge, or
    // at the far end.
    let tip = chord + (r * rule.length_share).min(reach - chord);
    let (px, py) = (near.x as f64 + ux * foot, near.y as f64 + uy * foot);
    let at = |along: f64, across: f64| {
        PointNm::new(
            (px + ux * along + nx * across).round() as i64,
            (py + uy * along + ny * across).round() as i64,
        )
    };
    Some(vec![at(0.0, base), at(tip, h), at(tip, -h), at(0.0, -base)])
}

#[cfg(test)]
#[path = "teardrop_tests.rs"]
mod tests;
