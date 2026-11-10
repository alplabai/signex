//! The board's own geometry (#2234): the outline, and what its sketch baked
//! to. Cutouts and mounting holes draw as openings in the board, keepouts as
//! hatched outlines. Rules: `docs/PCB_SKETCH.md`, "Drawing the board".
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, wgpu/WGSL public docs.

use signex_gfx::primitive::circle::Circle;
use signex_gfx::primitive::line::LineSegment;
use signex_gfx::primitive::polygon::GpuPolygon;
use signex_gfx::scene::Scene;
use signex_gfx::style::ColorSlot;
use signex_types::pcb::PcbBoard;

use super::emit::{point_to_xy, with_alpha_mul};
use crate::theme::ResolvedTheme;

/// How much fainter a keepout's hatch is than its outline.
const KEEPOUT_HATCH_ALPHA: f32 = 0.5;
/// Stroke widths in board mm.
const EDGE_WIDTH_MM: f32 = 0.15;
const HATCH_WIDTH_MM: f32 = 0.08;
/// Distance between hatch lines, raised for a large region so it never
/// draws more than `MAX_HATCH_LINES`.
const HATCH_PITCH_MM: f32 = 1.0;
const MAX_HATCH_LINES: f32 = 400.0;

/// A mounting hole as drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct HoleInput {
    pub center: [f32; 2],
    pub drill_mm: f32,
}

/// The outline, openings and keepouts of a board, in board mm.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoardFeaturesInput {
    /// The board outline, closing vertex not repeated; empty when unset.
    pub outline: Vec<[f32; 2]>,
    pub cutouts: Vec<Vec<[f32; 2]>>,
    pub holes: Vec<HoleInput>,
    pub keepouts: Vec<Vec<[f32; 2]>>,
}

impl BoardFeaturesInput {
    pub fn from_board(board: &PcbBoard) -> Self {
        fn polygon(points: &[signex_types::pcb::Point]) -> Vec<[f32; 2]> {
            points.iter().copied().map(point_to_xy).collect()
        }
        let mut features = Self {
            outline: polygon(&board.outline),
            ..Self::default()
        };
        if let Some(sketch) = board.sketch.as_ref() {
            features.cutouts = sketch
                .cutouts
                .iter()
                .map(|c| polygon(&c.boundary))
                .collect();
            features.keepouts = sketch
                .keepouts
                .iter()
                .map(|k| polygon(&k.boundary))
                .collect();
            features.holes = sketch
                .holes
                .iter()
                .map(|h| HoleInput {
                    center: point_to_xy(h.position),
                    drill_mm: h.drill as f32,
                })
                .collect();
        }
        features
    }
}

/// Append the board features to the overlay buckets of `scene`.
pub(super) fn emit(features: &BoardFeaturesInput, theme: &ResolvedTheme, scene: &mut Scene) {
    let edge = theme.color(ColorSlot::BoardEdge);
    let opening = theme.color(ColorSlot::BoardOpening);
    let keepout_color = theme.color(ColorSlot::Keepout);
    let keepout_hatch = with_alpha_mul(keepout_color, KEEPOUT_HATCH_ALPHA);
    for cutout in features.cutouts.iter().filter(|c| c.len() >= 3) {
        scene.overlay_polygons.push(GpuPolygon {
            vertices: cutout.clone(),
            holes: Vec::new(),
            fill_color: opening,
            stroke_color: Some(edge),
            stroke_width: EDGE_WIDTH_MM,
        });
    }
    for hole in features.holes.iter().filter(|h| h.drill_mm > 0.0) {
        let radius = hole.drill_mm * 0.5;
        // A filled disc (stroke width 0), then its rim.
        scene.overlay_circles.push(Circle {
            center: hole.center,
            radius,
            stroke_width: 0.0,
            color: opening,
        });
        scene.overlay_circles.push(Circle {
            center: hole.center,
            radius: (radius - EDGE_WIDTH_MM * 0.5).max(EDGE_WIDTH_MM * 0.5),
            stroke_width: EDGE_WIDTH_MM,
            color: edge,
        });
    }
    for keepout in features.keepouts.iter().filter(|k| k.len() >= 3) {
        push_loop(scene, keepout, keepout_color, EDGE_WIDTH_MM);
        for (p0, p1) in hatch(keepout) {
            scene
                .overlay_lines
                .push(line(p0, p1, keepout_hatch, HATCH_WIDTH_MM));
        }
    }
    if features.outline.len() >= 2 {
        push_loop(scene, &features.outline, edge, EDGE_WIDTH_MM);
    }
}

fn line(p0: [f32; 2], p1: [f32; 2], color: [f32; 4], width: f32) -> LineSegment {
    LineSegment {
        p0,
        p1,
        width,
        color,
        style: 0,
        _pad: 0,
    }
}

/// The closed loop through `points` as line segments.
fn push_loop(scene: &mut Scene, points: &[[f32; 2]], color: [f32; 4], width: f32) {
    for (i, &p0) in points.iter().enumerate() {
        let p1 = points[(i + 1) % points.len()];
        scene.overlay_lines.push(line(p0, p1, color, width));
    }
}

/// 45 degree hatch lines inside the closed `polygon` (even-odd), as
/// segments in board mm.
pub(super) fn hatch(polygon: &[[f32; 2]]) -> Vec<([f32; 2], [f32; 2])> {
    // Rotated frame: `u` runs along the hatch, `v` across it.
    let s = std::f32::consts::FRAC_1_SQRT_2;
    let rotated: Vec<(f32, f32)> = polygon
        .iter()
        .map(|p| ((p[0] + p[1]) * s, (p[1] - p[0]) * s))
        .collect();
    let (v_min, v_max) = rotated
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), &(_, v)| {
            (lo.min(v), hi.max(v))
        });
    if !(v_min.is_finite() && v_max.is_finite()) || v_max <= v_min {
        return Vec::new();
    }
    let pitch = HATCH_PITCH_MM.max((v_max - v_min) / MAX_HATCH_LINES);
    let mut out = Vec::new();
    let mut v = v_min + pitch * 0.5;
    while v < v_max {
        let mut crossings: Vec<f32> = Vec::new();
        for (i, &(ua, va)) in rotated.iter().enumerate() {
            let (ub, vb) = rotated[(i + 1) % rotated.len()];
            if (va <= v && v < vb) || (vb <= v && v < va) {
                crossings.push(ua + (v - va) / (vb - va) * (ub - ua));
            }
        }
        crossings.sort_by(f32::total_cmp);
        for pair in crossings.chunks_exact(2) {
            let back = |u: f32| [(u - v) * s, (u + v) * s];
            out.push((back(pair[0]), back(pair[1])));
        }
        v += pitch;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(size: f32) -> Vec<[f32; 2]> {
        vec![[0.0, 0.0], [size, 0.0], [size, size], [0.0, size]]
    }

    #[test]
    fn hatch_lines_stay_inside_the_region() {
        let lines = hatch(&square(10.0));
        assert!(!lines.is_empty());
        for (p0, p1) in lines {
            for p in [p0, p1] {
                assert!((-1e-3..=10.001).contains(&p[0]), "{p:?}");
                assert!((-1e-3..=10.001).contains(&p[1]), "{p:?}");
            }
        }
    }

    #[test]
    fn a_huge_region_is_hatched_with_a_bounded_number_of_lines() {
        assert!(hatch(&square(100_000.0)).len() <= MAX_HATCH_LINES as usize + 1);
    }

    #[test]
    fn a_degenerate_region_has_no_hatch() {
        assert!(hatch(&[[0.0, 0.0], [1.0, 1.0]]).is_empty());
        assert!(hatch(&[]).is_empty());
    }

    #[test]
    fn openings_and_keepouts_reach_the_overlay_buckets() {
        let features = BoardFeaturesInput {
            outline: square(50.0),
            cutouts: vec![square(5.0)],
            holes: vec![HoleInput {
                center: [4.0, 4.0],
                drill_mm: 3.2,
            }],
            keepouts: vec![square(10.0)],
        };
        let mut scene = Scene::default();
        emit(&features, &ResolvedTheme::builtin_default(), &mut scene);
        assert_eq!(scene.overlay_polygons.len(), 1, "the cutout");
        assert_eq!(scene.overlay_circles.len(), 2, "the hole and its rim");
        assert!(scene.overlay_circles[0].is_filled());
        // Four keepout edges, its hatch, and four outline edges.
        assert!(scene.overlay_lines.len() > 8);
    }
}
