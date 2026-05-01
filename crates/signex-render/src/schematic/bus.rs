//! Bus primitive — straight strokes, heavier than a wire.
//!
//! Buses render as a single line from `bus.start` to `bus.end` with
//! the active theme's `bus` colour and a stroke width of
//! [`BUS_STROKE_MM`] (≈ 3× the wire default — chosen so a bus reads
//! as visually distinct from a wire even at low zoom). See
//! `docs/RENDERING_RULES.md` (general — straight strokes section) and
//! `signex_types::schematic::Bus`.

use iced::widget::canvas::{Frame, Path, Stroke};
use signex_types::schematic::{Aabb, Bus, SelectedItem, SelectedKind};

use super::RenderContext;
use super::util::{aabbs_overlap, iced_color, point_finite};

/// Default bus stroke width — `0.45 mm` (= 18 mil), chosen as roughly
/// 3× the wire default so a single glance separates them visually.
pub const BUS_STROKE_MM: f64 = 0.45;

const SELECTION_WEIGHT_FACTOR: f64 = 1.5;

/// Render a single bus into the content layer's frame.
///
/// Frustum-culls against [`super::Viewport::visible_world_bounds`];
/// repaints with the theme's `selection` colour at +50% width when
/// the bus's UUID is in the snapshot's selection set.
pub fn draw_bus(frame: &mut Frame, bus: &Bus, ctx: &RenderContext<'_>) {
    let bbox = bus_aabb(bus);
    if !aabbs_overlap(&bbox, &ctx.viewport.visible_world_bounds()) {
        return;
    }

    let start = ctx.viewport.world_to_screen(bus.start);
    let end = ctx.viewport.world_to_screen(bus.end);
    if !point_finite(start) || !point_finite(end) {
        return;
    }

    let selected = ctx.is_selected(&SelectedItem::new(bus.uuid, SelectedKind::Bus));
    let width_world = BUS_STROKE_MM
        * if selected {
            SELECTION_WEIGHT_FACTOR
        } else {
            1.0
        };
    let width_px = (width_world * ctx.viewport.zoom_px_per_mm).max(1.0) as f32;

    let colour = if selected {
        iced_color(&ctx.theme().selection)
    } else {
        iced_color(&ctx.theme().bus)
    };

    frame.stroke(
        &Path::line(start, end),
        Stroke::default().with_width(width_px).with_color(colour),
    );
}

/// World-space AABB of a bus. `pub(crate)` for hit-test reuse.
#[inline]
pub(crate) fn bus_aabb(bus: &Bus) -> Aabb {
    Aabb::new(bus.start.x, bus.start.y, bus.end.x, bus.end.y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::Point;
    use uuid::Uuid;

    fn test_bus(start: Point, end: Point) -> Bus {
        Bus {
            uuid: Uuid::new_v4(),
            start,
            end,
        }
    }

    #[test]
    fn aabb_normalises_coordinate_order() {
        let b = test_bus(Point::new(5.0, 5.0), Point::new(-1.0, 1.0));
        let bbox = bus_aabb(&b);
        assert!(bbox.min_x <= bbox.max_x);
        assert!(bbox.min_y <= bbox.max_y);
    }

    #[test]
    fn bus_default_is_thicker_than_a_wire() {
        // Edge case: keep the visual hierarchy invariant in tests so
        // a future tuning of WIRE_DEFAULT_STROKE_MM doesn't silently
        // make wires fatter than buses.
        assert!(BUS_STROKE_MM > super::super::wire::WIRE_DEFAULT_STROKE_MM);
    }
}
