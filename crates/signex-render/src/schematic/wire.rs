//! Wire primitive — straight strokes between two endpoints.
//!
//! Wires render as a single line from `wire.start` to `wire.end` using
//! the active theme's `wire` colour and `wire.stroke_width` (or the
//! schematic default when `0.0`). Selected wires render at +50% width
//! in the theme's `selection` colour. See
//! `docs/RENDERING_RULES.md` (general — straight strokes section) and
//! `signex_types::schematic::Wire`.

use iced::widget::canvas::{Frame, Path, Stroke};
use signex_types::schematic::{Aabb, SelectedItem, SelectedKind, Wire};

use super::RenderContext;
use super::util::{aabbs_overlap, iced_color, point_finite};

/// Default wire stroke width when `wire.stroke_width == 0.0`. Chosen
/// to match the visual weight of legacy schematic renderings (~0.15 mm
/// / 6 mil) without crowding the page at high zoom.
pub const WIRE_DEFAULT_STROKE_MM: f64 = 0.15;

/// Multiplier applied to a wire's stroke width when it is part of the
/// active selection. `1.5×` matches the Altium-parity emphasis used
/// for symbol body strokes.
const SELECTION_WEIGHT_FACTOR: f64 = 1.5;

/// Render a single wire into the content layer's frame.
///
/// Skips wires whose AABB lies entirely outside the viewport's visible
/// world bounds (Q9 (c) frustum-culling improvement); selected wires
/// repaint with the theme's selection colour and a heavier stroke.
///
/// # Example
///
/// ```ignore
/// for wire in &snapshot.sheet.wires {
///     wire::draw_wire(frame, wire, &ctx);
/// }
/// ```
pub fn draw_wire(frame: &mut Frame, wire: &Wire, ctx: &RenderContext<'_>) {
    let bbox = wire_aabb(wire);
    let visible = ctx.visible_world_bounds();
    if !aabbs_overlap(&bbox, &visible) {
        return;
    }

    let start = ctx.viewport.world_to_screen(wire.start);
    let end = ctx.viewport.world_to_screen(wire.end);
    if !point_finite(start) || !point_finite(end) {
        return;
    }

    let selected = ctx.is_selected(&SelectedItem::new(wire.uuid, SelectedKind::Wire));
    let width_world = effective_stroke_mm(wire)
        * if selected {
            SELECTION_WEIGHT_FACTOR
        } else {
            1.0
        };
    let width_px = (width_world * ctx.viewport.zoom_px_per_mm()).max(1.0) as f32;

    let colour = if selected {
        iced_color(&ctx.theme().selection)
    } else {
        iced_color(&ctx.theme().wire)
    };

    let path = Path::line(start, end);
    let stroke = Stroke::default().with_width(width_px).with_color(colour);
    frame.stroke(&path, stroke);
}

/// World-space axis-aligned bounding box of a wire. `pub(crate)` so
/// `super::hit_test` (Wave 4) can reuse the same bbox the renderer
/// uses for culling — keeps spatial-hash buckets and visible-bounds
/// culling consistent.
#[inline]
pub(crate) fn wire_aabb(wire: &Wire) -> Aabb {
    Aabb::new(wire.start.x, wire.start.y, wire.end.x, wire.end.y)
}

/// Apply the schematic default when the wire's stored stroke width is
/// effectively zero.
#[inline]
fn effective_stroke_mm(wire: &Wire) -> f64 {
    if wire.stroke_width > 0.0 {
        wire.stroke_width
    } else {
        WIRE_DEFAULT_STROKE_MM
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::Point;
    use uuid::Uuid;

    fn test_wire(start: Point, end: Point) -> Wire {
        Wire {
            uuid: Uuid::new_v4(),
            start,
            end,
            stroke_width: 0.0,
        }
    }

    #[test]
    fn aabb_includes_both_endpoints() {
        let w = test_wire(Point::new(2.0, -1.0), Point::new(-3.0, 4.0));
        let bbox = wire_aabb(&w);
        assert!(bbox.contains(2.0, -1.0));
        assert!(bbox.contains(-3.0, 4.0));
        assert!(bbox.contains(0.0, 0.0));
    }

    #[test]
    fn effective_stroke_falls_back_to_default_when_zero() {
        let w = test_wire(Point::new(0.0, 0.0), Point::new(1.0, 0.0));
        assert!((effective_stroke_mm(&w) - WIRE_DEFAULT_STROKE_MM).abs() < f64::EPSILON);
    }

    #[test]
    fn effective_stroke_honours_explicit_user_width() {
        let mut w = test_wire(Point::new(0.0, 0.0), Point::new(1.0, 0.0));
        w.stroke_width = 0.42;
        assert!((effective_stroke_mm(&w) - 0.42).abs() < f64::EPSILON);
    }
}
