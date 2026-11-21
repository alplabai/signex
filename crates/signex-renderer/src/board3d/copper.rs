//! The board's laminate: its outline, the openings through it and its
//! walls. The layers on it are in `layers`.

use std::ops::Range;

use signex_gfx::ring_fill::{disjoint_holes, retain_holes_inside};
use signex_gfx::scene3d::Scene3d;
use signex_types::pcb::PadType;

use super::stack::StackHeights;
use super::{Board3dColors, board_xy};
use crate::pcb::PcbSnapshot;

/// Points round a drill hole or a via.
pub(super) const ROUND_SEGMENTS: usize = 16;
/// Points round each end of a track.
const CAP_SEGMENTS: usize = 8;
/// Space round the copper when the board has no outline, mm.
const NO_OUTLINE_MARGIN_MM: f32 = 1.0;

/// The laminate between its two copper planes: its outline (or, with none,
/// the box round the copper) less the cutouts, mounting holes, pad drills
/// and via drills that lie inside it. The walls run the board's full
/// thickness, FR-4 on the edge and in unplated holes, plated in a plated
/// drill. Empty when there is nothing to frame. A hole that meets one cut
/// before it (two vias at one point, a drill in a cutout) is left out, so
/// one such pair cannot cost every hole; the second value counts them.
pub(super) fn append_board_slab(
    scene: &mut Scene3d,
    every: &PcbSnapshot,
    heights: &StackHeights,
    colors: &Board3dColors,
) -> (Range<u32>, usize) {
    let start = scene.indices.len() as u32;
    let Some(outline) = slab_outline(every) else {
        return (start..start, 0);
    };
    let (openings, dropped) = slab_openings(every, &outline);
    let holes: Vec<Vec<[f32; 2]>> = openings.iter().map(Opening::ring).collect();
    for z in [heights.bottom_core, heights.top_core] {
        scene.append_face(&outline, &holes, z, colors.substrate);
    }
    scene.append_walls(&outline, 0.0, heights.thickness, colors.substrate);
    for (opening, ring) in openings.iter().zip(&holes) {
        let wall = if opening.is_plated() {
            colors.finish
        } else {
            colors.substrate
        };
        scene.append_walls(ring, 0.0, heights.thickness, wall);
    }
    (start..scene.indices.len() as u32, dropped)
}

/// An opening through the board slab, in the 3D frame.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Opening {
    /// A cutout, closing point not repeated.
    Cutout(Vec<[f32; 2]>),
    /// A mounting hole, pad drill or via drill.
    Drill {
        center: [f32; 2],
        diameter: f32,
        /// A via or plated pad's drill: its wall is metal.
        plated: bool,
    },
}

impl Opening {
    /// The opening as a ring: a drill as a circle of `ROUND_SEGMENTS`
    /// points.
    pub(super) fn ring(&self) -> Vec<[f32; 2]> {
        match self {
            Self::Cutout(ring) => ring.clone(),
            Self::Drill {
                center, diameter, ..
            } => circle(*center, diameter * 0.5, ROUND_SEGMENTS),
        }
    }

    fn is_plated(&self) -> bool {
        matches!(self, Self::Drill { plated: true, .. })
    }
}

/// The slab's outline in the 3D frame: the board outline, or with none the
/// box round the copper; `None` when there is nothing to frame.
pub(super) fn slab_outline(every: &PcbSnapshot) -> Option<Vec<[f32; 2]>> {
    let features = &every.board_features;
    if features.outline.len() >= 3 {
        Some(features.outline.iter().copied().map(board_xy).collect())
    } else {
        copper_box(every)
    }
}

/// The cutouts, mounting holes, pad drills and via drills that lie inside
/// `outline`, less each that meets one before it (as rings,
/// `disjoint_holes`); the second value counts those.
pub(super) fn slab_openings(every: &PcbSnapshot, outline: &[[f32; 2]]) -> (Vec<Opening>, usize) {
    let features = &every.board_features;
    let cutouts = features
        .cutouts
        .iter()
        .filter(|cutout| cutout.len() >= 3)
        .map(|cutout| Opening::Cutout(cutout.iter().copied().map(board_xy).collect()));
    let drills = features
        .holes
        .iter()
        .map(|hole| (hole.center, hole.drill_mm, false))
        .chain(
            every
                .pads
                .iter()
                .map(|pad| (pad.center, pad.drill_mm, pad.pad_type == PadType::Thru)),
        )
        .chain(
            every
                .vias
                .iter()
                .map(|via| (via.center, via.drill_mm, true)),
        )
        .filter(|(_, drill, _)| drill.is_finite() && *drill > 0.0)
        .map(|(center, drill, plated)| Opening::Drill {
            center: board_xy(center),
            diameter: drill,
            plated,
        });
    let mut openings: Vec<Opening> = cutouts.chain(drills).collect();
    let mut rings: Vec<Vec<[f32; 2]>> = openings.iter().map(Opening::ring).collect();
    let inside: Vec<bool> = rings
        .iter()
        .map(|ring| {
            let mut one = vec![ring.clone()];
            retain_holes_inside(outline, &mut one);
            !one.is_empty()
        })
        .collect();
    let mut keep = inside.iter();
    openings.retain(|_| keep.next().copied().unwrap_or(false));
    let mut keep = inside.iter();
    rings.retain(|_| keep.next().copied().unwrap_or(false));
    let (kept, dropped) = disjoint_holes(rings.clone());
    // `kept` is `rings` in order less the dropped ones: walk both.
    let mut next = kept.iter().peekable();
    let survived: Vec<bool> = rings
        .iter()
        .map(|ring| {
            let hit = next.peek().is_some_and(|k| *k == ring);
            if hit {
                next.next();
            }
            hit
        })
        .collect();
    let mut keep = survived.iter();
    openings.retain(|_| keep.next().copied().unwrap_or(false));
    (openings, dropped)
}

/// A circle of `segments` points, counter-clockwise.
pub(super) fn circle(center: [f32; 2], radius: f32, segments: usize) -> Vec<[f32; 2]> {
    (0..segments)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / segments as f32;
            [center[0] + radius * a.cos(), center[1] + radius * a.sin()]
        })
        .collect()
}

/// The outline of a track from `a` to `b`, `radius` half its width, with
/// round ends.
pub(super) fn capsule(a: [f32; 2], b: [f32; 2], radius: f32) -> Vec<[f32; 2]> {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = (dx * dx + dy * dy).sqrt();
    let heading = if len > 0.0 { dy.atan2(dx) } else { 0.0 };
    let half_turn = std::f32::consts::PI;
    let mut points = Vec::with_capacity(2 * (CAP_SEGMENTS + 1));
    // Round the `b` end from one side to the other, then the `a` end.
    for (end, start) in [
        (b, heading - half_turn / 2.0),
        (a, heading + half_turn / 2.0),
    ] {
        for i in 0..=CAP_SEGMENTS {
            let t = start + half_turn * i as f32 / CAP_SEGMENTS as f32;
            points.push([end[0] + radius * t.cos(), end[1] + radius * t.sin()]);
        }
    }
    points
}

/// The box round the copper, with a margin, as a ring; `None` with none.
fn copper_box(every: &PcbSnapshot) -> Option<Vec<[f32; 2]>> {
    let points = every
        .pads
        .iter()
        .flat_map(|pad| pad.outline.iter().copied())
        .chain(every.traces.iter().flat_map(|t| [t.p0, t.p1]))
        .chain(every.vias.iter().map(|v| v.center))
        .chain(every.zones.iter().flat_map(|z| z.vertices.iter().copied()))
        .filter(|p| p[0].is_finite() && p[1].is_finite())
        .map(board_xy);
    let mut extent: Option<([f32; 2], [f32; 2])> = None;
    for p in points {
        extent = Some(match extent {
            None => (p, p),
            Some((lo, hi)) => (
                [lo[0].min(p[0]), lo[1].min(p[1])],
                [hi[0].max(p[0]), hi[1].max(p[1])],
            ),
        });
    }
    let (lo, hi) = extent?;
    let m = NO_OUTLINE_MARGIN_MM;
    Some(vec![
        [lo[0] - m, lo[1] - m],
        [hi[0] + m, lo[1] - m],
        [hi[0] + m, hi[1] + m],
        [lo[0] - m, hi[1] + m],
    ])
}
