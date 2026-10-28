//! Viewport culling for the PCB CPU canvas.
//!
//! The scene is camera-independent world-space geometry, so each primitive's
//! world extent is computed once per scene ([`SceneBounds::from_scene`]) and a
//! pan/zoom redraw only tests those rectangles against the visible world rect
//! instead of stroking every primitive.

use signex_gfx::scene::Scene;

/// Axis-aligned world-space (mm) rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct WorldRect {
    min: [f32; 2],
    max: [f32; 2],
}

impl WorldRect {
    fn from_points<'a>(points: impl IntoIterator<Item = &'a [f32; 2]>, pad: f32) -> Self {
        let mut rect = Self {
            min: [f32::INFINITY; 2],
            max: [f32::NEG_INFINITY; 2],
        };
        for p in points {
            rect.min = [rect.min[0].min(p[0]), rect.min[1].min(p[1])];
            rect.max = [rect.max[0].max(p[0]), rect.max[1].max(p[1])];
        }
        rect.grown(pad)
    }

    fn grown(self, pad: f32) -> Self {
        Self {
            min: [self.min[0] - pad, self.min[1] - pad],
            max: [self.max[0] + pad, self.max[1] + pad],
        }
    }

    /// Non-finite extents (NaN/inf geometry, or an empty primitive) can't be
    /// ordered against the view, so they are always kept: culling must never
    /// change what is drawn.
    fn is_finite(&self) -> bool {
        self.min.iter().chain(&self.max).all(|v| v.is_finite())
    }

    fn intersects(&self, view: &Self) -> bool {
        !self.is_finite()
            || (self.min[0] <= view.max[0]
                && self.max[0] >= view.min[0]
                && self.min[1] <= view.max[1]
                && self.max[1] >= view.min[1])
    }

    /// The world rect shown in a `size`-pixel frame under `camera`, grown by
    /// `margin_px` of screen space (covers the minimum 0.5 px stroke/radius
    /// floors and anti-aliasing). `None` when the camera is degenerate, in
    /// which case nothing is culled.
    pub(super) fn visible(
        camera: &crate::canvas::Camera,
        size: iced::Size,
        margin_px: f32,
    ) -> Option<Self> {
        if !(camera.scale.is_finite() && camera.scale > 0.0) {
            return None;
        }
        let world = |x: f32, y: f32| {
            [
                (x - camera.offset.x) / camera.scale,
                (y - camera.offset.y) / camera.scale,
            ]
        };
        let a = world(0.0, 0.0);
        let b = world(size.width, size.height);
        Some(
            Self {
                min: [a[0].min(b[0]), a[1].min(b[1])],
                max: [a[0].max(b[0]), a[1].max(b[1])],
            }
            .grown(margin_px / camera.scale),
        )
    }
}

/// Screen-space slack added around the viewport before testing primitives.
pub(super) const CULL_MARGIN_PX: f32 = 2.0;

/// Per-bucket world extents, index-parallel with the scene's primitive Vecs.
#[derive(Debug, Default)]
pub(super) struct SceneBounds {
    pub(super) lines: Vec<WorldRect>,
    pub(super) circles: Vec<WorldRect>,
    pub(super) polygons: Vec<WorldRect>,
    pub(super) top_polygons: Vec<WorldRect>,
    pub(super) overlay_lines: Vec<WorldRect>,
    pub(super) overlay_circles: Vec<WorldRect>,
    pub(super) overlay_polygons: Vec<WorldRect>,
}

fn line_bounds(lines: &[signex_gfx::primitive::line::LineSegment]) -> Vec<WorldRect> {
    lines
        .iter()
        .map(|l| WorldRect::from_points([&l.p0, &l.p1], l.width.max(0.0)))
        .collect()
}

fn circle_bounds(circles: &[signex_gfx::primitive::circle::Circle]) -> Vec<WorldRect> {
    circles
        .iter()
        .map(|c| {
            let stroke = if c.is_filled() {
                0.0
            } else {
                c.stroke_width.max(0.0)
            };
            WorldRect::from_points([&c.center], c.radius.max(0.0) + stroke)
        })
        .collect()
}

fn polygon_bounds(polygons: &[signex_gfx::primitive::polygon::GpuPolygon]) -> Vec<WorldRect> {
    polygons
        .iter()
        .map(|p| {
            let stroke = if p.is_stroked() {
                p.stroke_width.max(0.0)
            } else {
                0.0
            };
            // Holes lie inside the outer ring for a valid polygon, but include
            // them anyway so a malformed ring can't be wrongly culled.
            WorldRect::from_points(p.vertices.iter().chain(p.holes.iter().flatten()), stroke)
        })
        .collect()
}

impl SceneBounds {
    pub(super) fn from_scene(scene: &Scene) -> Self {
        Self {
            lines: line_bounds(&scene.lines),
            circles: circle_bounds(&scene.circles),
            polygons: polygon_bounds(&scene.polygons),
            top_polygons: polygon_bounds(&scene.top_polygons),
            overlay_lines: line_bounds(&scene.overlay_lines),
            overlay_circles: circle_bounds(&scene.overlay_circles),
            overlay_polygons: polygon_bounds(&scene.overlay_polygons),
        }
    }

    /// Number of primitives (all drawn buckets) that survive `view`.
    #[cfg(test)]
    pub(super) fn visible_count(&self, view: Option<&WorldRect>) -> usize {
        [
            &self.lines,
            &self.circles,
            &self.polygons,
            &self.top_polygons,
            &self.overlay_lines,
            &self.overlay_circles,
            &self.overlay_polygons,
        ]
        .iter()
        .map(|b| {
            b.iter()
                .filter(|r| view.is_none_or(|v| r.intersects(v)))
                .count()
        })
        .sum()
    }
}

/// The primitives of `items` whose `bounds` intersect `view`, in draw order.
/// `view == None` keeps everything.
pub(super) fn visible<'a, T>(
    items: &'a [T],
    bounds: &'a [WorldRect],
    view: Option<WorldRect>,
) -> impl Iterator<Item = &'a T> {
    debug_assert_eq!(items.len(), bounds.len());
    items
        .iter()
        .zip(bounds)
        .filter(move |(_, b)| view.as_ref().is_none_or(|v| b.intersects(v)))
        .map(|(item, _)| item)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::Camera;
    use signex_gfx::primitive::circle::Circle;
    use signex_gfx::primitive::line::LineSegment;
    use signex_gfx::primitive::polygon::GpuPolygon;

    fn line(p0: [f32; 2], p1: [f32; 2]) -> LineSegment {
        LineSegment {
            p0,
            p1,
            width: 0.2,
            color: [1.0; 4],
            style: 0,
            _pad: 0,
        }
    }

    fn circle(center: [f32; 2], radius: f32) -> Circle {
        Circle {
            center,
            radius,
            stroke_width: 0.1,
            color: [1.0; 4],
        }
    }

    fn scene() -> Scene {
        let mut scene = Scene::default();
        // Inside a 100x100 px frame at scale 1, offset 0 -> world 0..100.
        scene.lines.push(line([10.0, 10.0], [20.0, 20.0]));
        // Far outside.
        scene.lines.push(line([500.0, 500.0], [510.0, 500.0]));
        // Straddles the right edge (x = 100).
        scene.lines.push(line([90.0, 50.0], [150.0, 50.0]));
        scene.circles.push(circle([50.0, 50.0], 2.0));
        scene.circles.push(circle([-300.0, 50.0], 2.0));
        // Centre outside, but radius reaches in.
        scene.circles.push(circle([101.0, 50.0], 3.0));
        scene.polygons.push(GpuPolygon {
            vertices: vec![[40.0, 40.0], [60.0, 40.0], [50.0, 60.0]],
            ..Default::default()
        });
        scene.overlay_polygons.push(GpuPolygon {
            vertices: vec![[900.0, 900.0], [910.0, 900.0], [905.0, 910.0]],
            ..Default::default()
        });
        scene
    }

    fn view_of(camera: &Camera, w: f32, h: f32) -> Option<WorldRect> {
        WorldRect::visible(camera, iced::Size::new(w, h), CULL_MARGIN_PX)
    }

    fn unit_camera() -> Camera {
        Camera {
            offset: iced::Point::new(0.0, 0.0),
            scale: 1.0,
        }
    }

    #[test]
    fn only_primitives_in_the_viewport_survive() {
        let scene = scene();
        let bounds = SceneBounds::from_scene(&scene);
        let view = view_of(&unit_camera(), 100.0, 100.0);

        let lines: Vec<_> = visible(&scene.lines, &bounds.lines, view).collect();
        assert_eq!(lines.len(), 2, "the far line is culled");
        assert!(lines.iter().all(|l| l.p0[0] < 200.0));

        let circles: Vec<_> = visible(&scene.circles, &bounds.circles, view).collect();
        assert_eq!(circles.len(), 2, "the far circle is culled");

        assert_eq!(visible(&scene.polygons, &bounds.polygons, view).count(), 1);
        assert_eq!(
            visible(&scene.overlay_polygons, &bounds.overlay_polygons, view).count(),
            0
        );
    }

    #[test]
    fn a_primitive_straddling_the_edge_is_kept() {
        let scene = scene();
        let bounds = SceneBounds::from_scene(&scene);
        let view = view_of(&unit_camera(), 100.0, 100.0);
        // The line from x=90 to x=150 crosses the right edge; the circle whose
        // centre is outside (x=101) reaches in via its radius.
        assert!(visible(&scene.lines, &bounds.lines, view).any(|l| l.p1[0] == 150.0));
        assert!(visible(&scene.circles, &bounds.circles, view).any(|c| c.center[0] == 101.0));
    }

    #[test]
    fn stroke_width_extends_the_bounds() {
        let mut scene = Scene::default();
        // Wide line just outside x=100 whose half-width reaches the frame.
        let mut wide = line([103.0, 50.0], [110.0, 50.0]);
        wide.width = 4.0;
        scene.lines.push(wide);
        let bounds = SceneBounds::from_scene(&scene);
        let view = view_of(&unit_camera(), 100.0, 100.0);
        assert_eq!(visible(&scene.lines, &bounds.lines, view).count(), 1);
    }

    #[test]
    fn zooming_out_to_fit_keeps_everything() {
        let scene = scene();
        let bounds = SceneBounds::from_scene(&scene);
        let total = bounds.visible_count(None);
        assert_eq!(total, 8);
        // 100 px frame showing 2000 mm (world -1000..1000).
        let camera = Camera {
            offset: iced::Point::new(50.0, 50.0),
            scale: 0.05,
        };
        let view = view_of(&camera, 100.0, 100.0);
        assert_eq!(bounds.visible_count(view.as_ref()), total);
    }

    #[test]
    fn panning_changes_the_visible_set() {
        let scene = scene();
        let bounds = SceneBounds::from_scene(&scene);
        let at_origin = bounds.visible_count(view_of(&unit_camera(), 100.0, 100.0).as_ref());
        // Pan so the world window is x 450..550, y 450..550 (the far line).
        let panned = Camera {
            offset: iced::Point::new(-450.0, -450.0),
            scale: 1.0,
        };
        let there = bounds.visible_count(view_of(&panned, 100.0, 100.0).as_ref());
        assert_eq!(at_origin, 5);
        assert_eq!(there, 1);
    }

    #[test]
    fn degenerate_camera_and_nonfinite_geometry_are_never_culled() {
        let bad = Camera {
            offset: iced::Point::new(0.0, 0.0),
            scale: 0.0,
        };
        assert!(view_of(&bad, 100.0, 100.0).is_none());

        let mut scene = Scene::default();
        scene.lines.push(line([f32::NAN, 0.0], [1.0, 1.0]));
        let bounds = SceneBounds::from_scene(&scene);
        let view = view_of(&unit_camera(), 10.0, 10.0);
        assert_eq!(visible(&scene.lines, &bounds.lines, view).count(), 1);
    }
}
