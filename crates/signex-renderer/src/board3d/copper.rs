//! The board slab and the copper on its faces.

use std::ops::Range;

use signex_gfx::ring_fill::{disjoint_holes, retain_holes_inside};
use signex_gfx::scene3d::Scene3d;
use signex_types::pcb_layer::CopperLayer;

use super::{Board3dColors, COPPER_LIFT_MM, Face, board_xy};
use crate::pcb::{CopperInk, PcbSnapshot};

/// Points round a drill hole or a via.
const ROUND_SEGMENTS: usize = 16;
/// Points round each end of a track.
const CAP_SEGMENTS: usize = 8;
/// Space round the copper when the board has no outline, mm.
const NO_OUTLINE_MARGIN_MM: f32 = 1.0;
/// Copper kinds sit at slightly different heights so overlapping copper
/// never fights in the depth buffer: zones lowest, pads and vias highest.
const ZONE_LIFT: f32 = COPPER_LIFT_MM;
const TRACK_LIFT: f32 = COPPER_LIFT_MM * 1.3;
const PAD_LIFT: f32 = COPPER_LIFT_MM * 1.6;

/// The board from 0 to `thickness`: its outline (or, with none, the box
/// round the copper) less the cutouts, mounting holes, pad drills and via
/// drills that lie inside it. Empty when there is nothing to frame. A hole
/// that meets one cut before it (two vias at one point, a drill in a
/// cutout) is left out, so one such pair cannot cost every hole; the
/// second value counts them.
pub(super) fn append_board_slab(
    scene: &mut Scene3d,
    every: &PcbSnapshot,
    thickness: f32,
    colors: &Board3dColors,
) -> (Range<u32>, usize) {
    let Some(outline) = slab_outline(every) else {
        return (scene.indices.len() as u32..scene.indices.len() as u32, 0);
    };
    let (openings, dropped) = slab_openings(every, &outline);
    let holes: Vec<Vec<[f32; 2]>> = openings.iter().map(Opening::ring).collect();
    let range = scene.append_prism(&outline, &holes, (0.0, thickness), colors.mask, colors.edge);
    (range, dropped)
}

/// An opening through the board slab, in the 3D frame.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Opening {
    /// A cutout, closing point not repeated.
    Cutout(Vec<[f32; 2]>),
    /// A mounting hole, pad drill or via drill.
    Drill { center: [f32; 2], diameter: f32 },
}

impl Opening {
    /// The opening as a ring: a drill as a circle of `ROUND_SEGMENTS`
    /// points.
    pub(super) fn ring(&self) -> Vec<[f32; 2]> {
        match self {
            Self::Cutout(ring) => ring.clone(),
            Self::Drill { center, diameter } => circle(*center, diameter * 0.5, ROUND_SEGMENTS),
        }
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
        .map(|hole| (hole.center, hole.drill_mm))
        .chain(every.pads.iter().map(|pad| (pad.center, pad.drill_mm)))
        .chain(every.vias.iter().map(|via| (via.center, via.drill_mm)))
        .filter(|(_, drill)| drill.is_finite() && *drill > 0.0)
        .map(|(center, drill)| Opening::Drill {
            center: board_xy(center),
            diameter: drill,
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

/// The copper `shown` holds on the two faces: tracks, pads, vias and
/// zones, each on the face of its layer; a via or a through-hole pad on
/// both. Inner copper is inside the board and is not drawn.
pub(super) fn append_copper(
    scene: &mut Scene3d,
    shown: &PcbSnapshot,
    thickness: f32,
    colors: &Board3dColors,
) {
    let put = |scene: &mut Scene3d,
               ink: CopperInk,
               lift: f32,
               outline: &[[f32; 2]],
               holes: &[Vec<[f32; 2]>]| {
        for (face, color) in faces(ink, colors) {
            let z = match face {
                Face::Top => thickness + lift,
                Face::Bottom => -lift,
            };
            scene.append_face(outline, holes, z, color);
        }
    };
    for zone in &shown.zones {
        let outline: Vec<[f32; 2]> = zone.vertices.iter().copied().map(board_xy).collect();
        let holes: Vec<Vec<[f32; 2]>> = zone
            .holes
            .iter()
            .map(|hole| hole.iter().copied().map(board_xy).collect())
            .collect();
        put(scene, zone.ink, ZONE_LIFT, &outline, &holes);
    }
    for trace in &shown.traces {
        let outline = capsule(board_xy(trace.p0), board_xy(trace.p1), trace.width_mm * 0.5);
        put(scene, trace.ink, TRACK_LIFT, &outline, &[]);
    }
    for pad in &shown.pads {
        let outline: Vec<[f32; 2]> = pad.outline.iter().copied().map(board_xy).collect();
        let holes = drill_hole(pad.center, pad.drill_mm);
        put(scene, pad.ink, PAD_LIFT, &outline, &holes);
    }
    for via in &shown.vias {
        let outline = circle(board_xy(via.center), via.diameter_mm * 0.5, ROUND_SEGMENTS);
        let holes = drill_hole(via.center, via.drill_mm);
        put(scene, via.ink, PAD_LIFT, &outline, &holes);
    }
}

/// The faces copper of `ink` lies on, with its colour.
fn faces(ink: CopperInk, colors: &Board3dColors) -> Vec<(Face, [f32; 4])> {
    match ink {
        CopperInk::Layer(CopperLayer::Top) => vec![(Face::Top, colors.top_copper)],
        CopperInk::Layer(CopperLayer::Bottom) => vec![(Face::Bottom, colors.bottom_copper)],
        CopperInk::MultiLayer => vec![
            (Face::Top, colors.multi_layer),
            (Face::Bottom, colors.multi_layer),
        ],
        CopperInk::Layer(CopperLayer::Inner(_)) | CopperInk::Unknown => Vec::new(),
    }
}

fn drill_hole(center: [f32; 2], drill_mm: f32) -> Vec<Vec<[f32; 2]>> {
    if drill_mm.is_finite() && drill_mm > 0.0 {
        vec![circle(board_xy(center), drill_mm * 0.5, ROUND_SEGMENTS)]
    } else {
        Vec::new()
    }
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
fn capsule(a: [f32; 2], b: [f32; 2], radius: f32) -> Vec<[f32; 2]> {
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
