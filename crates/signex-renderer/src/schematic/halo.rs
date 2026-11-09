//! Selection halo: a padded, rounded box with a soft fill and a dashed
//! outline, lowered to the scene's overlay polygon and line buckets.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

use signex_gfx::primitive::line::LineSegment;
use signex_gfx::primitive::polygon::GpuPolygon;
use signex_gfx::scene::Scene;

/// Segments per rounded corner.
const CORNER_SEGMENTS: usize = 6;

/// A selection halo around the box `min`..`max` (world mm). Padding, corner
/// radius and stroke width are world mm too: the caller converts its screen
/// px sizes at the current zoom.
#[derive(Clone, Debug, PartialEq)]
pub struct OverlayHaloInput {
    pub min: [f32; 2],
    pub max: [f32; 2],
    pub pad_mm: f32,
    pub radius_mm: f32,
    pub stroke_width_mm: f32,
    pub fill_color: [f32; 4],
    pub stroke_color: [f32; 4],
}

impl OverlayHaloInput {
    /// The rounded outline of the padded box: four corner arcs, clockwise
    /// from the top-right one.
    pub fn outline(&self) -> HaloOutline {
        let (x0, y0) = (self.min[0] - self.pad_mm, self.min[1] - self.pad_mm);
        let (x1, y1) = (self.max[0] + self.pad_mm, self.max[1] + self.pad_mm);
        let r = self
            .radius_mm
            .min((x1 - x0) * 0.5)
            .min((y1 - y0) * 0.5)
            .max(0.0);
        // Corner centres and the angle each arc starts at (screen y down).
        let corners = [
            ([x1 - r, y0 + r], -std::f32::consts::FRAC_PI_2),
            ([x1 - r, y1 - r], 0.0),
            ([x0 + r, y1 - r], std::f32::consts::FRAC_PI_2),
            ([x0 + r, y0 + r], std::f32::consts::PI),
        ];
        let arcs = corners.map(|(c, start)| {
            (0..=CORNER_SEGMENTS)
                .map(|i| {
                    let a =
                        start + std::f32::consts::FRAC_PI_2 * (i as f32 / CORNER_SEGMENTS as f32);
                    [c[0] + a.cos() * r, c[1] + a.sin() * r]
                })
                .collect::<Vec<_>>()
        });
        HaloOutline { arcs }
    }
}

/// The four corner arcs of a halo, top-right first, clockwise. Each side is
/// the straight run from one arc's last point to the next arc's first.
#[derive(Clone, Debug, PartialEq)]
pub struct HaloOutline {
    pub arcs: [Vec<[f32; 2]>; 4],
}

impl HaloOutline {
    /// Every outline vertex in order: the polygon boundary.
    pub fn vertices(&self) -> Vec<[f32; 2]> {
        self.arcs.iter().flatten().copied().collect()
    }

    /// The four straight sides as `(from, to)`.
    pub fn sides(&self) -> [([f32; 2], [f32; 2]); 4] {
        std::array::from_fn(|i| {
            let from = *self.arcs[i].last().expect("an arc has points");
            let to = self.arcs[(i + 1) % 4][0];
            (from, to)
        })
    }
}

/// Lower `halos` into the overlay buckets: a filled polygon, then a dashed
/// run per straight side and a solid chord run per corner.
pub(super) fn emit_halos(halos: &[OverlayHaloInput], scene: &mut Scene) {
    for halo in halos {
        let outline = halo.outline();
        scene.overlay_polygons.push(GpuPolygon {
            vertices: outline.vertices(),
            holes: Vec::new(),
            fill_color: halo.fill_color,
            stroke_color: None,
            stroke_width: 0.0,
        });
        let line = |p0: [f32; 2], p1: [f32; 2], style: u32| LineSegment {
            p0,
            p1,
            width: halo.stroke_width_mm,
            color: halo.stroke_color,
            style,
            _pad: 0,
        };
        for (from, to) in outline.sides() {
            scene
                .overlay_lines
                .push(line(from, to, LineSegment::STYLE_DASHED));
        }
        for arc in &outline.arcs {
            for pair in arc.windows(2) {
                scene.overlay_lines.push(line(pair[0], pair[1], 0));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn halo() -> OverlayHaloInput {
        OverlayHaloInput {
            min: [10.0, 20.0],
            max: [30.0, 40.0],
            pad_mm: 2.0,
            radius_mm: 1.5,
            stroke_width_mm: 0.2,
            fill_color: [1.0, 0.5, 0.0, 0.5],
            stroke_color: [1.0, 0.0, 0.0, 1.0],
        }
    }

    #[test]
    fn the_outline_is_the_padded_box_with_rounded_corners() {
        let v = halo().outline().vertices();
        let (min_x, max_x) = v.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| {
            (lo.min(p[0]), hi.max(p[0]))
        });
        assert!((min_x - 8.0).abs() < 1e-4 && (max_x - 32.0).abs() < 1e-4);
        // No vertex sits on the box corner: it is cut by the radius.
        assert!(v.iter().all(|p| !(p[0] == 8.0 && p[1] == 18.0)));
        assert_eq!(v.len(), 4 * (CORNER_SEGMENTS + 1));
    }

    #[test]
    fn a_halo_lowers_to_one_fill_and_a_dashed_side_each() {
        let mut scene = Scene::default();
        emit_halos(&[halo()], &mut scene);
        assert_eq!(scene.overlay_polygons.len(), 1);
        let poly = &scene.overlay_polygons[0];
        assert_eq!(poly.fill_color, [1.0, 0.5, 0.0, 0.5]);
        assert_eq!(poly.stroke_color, None);
        let dashed = scene.overlay_lines.iter().filter(|l| l.is_dashed()).count();
        assert_eq!(dashed, 4);
        assert_eq!(scene.overlay_lines.len(), 4 + 4 * CORNER_SEGMENTS);
    }

    #[test]
    fn an_oversize_radius_is_clamped_to_half_the_short_side() {
        let h = OverlayHaloInput {
            radius_mm: 99.0,
            ..halo()
        };
        let v = h.outline().vertices();
        assert!(v.iter().all(|p| p[0].is_finite() && p[1].is_finite()));
    }
}
