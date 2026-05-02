//! Bus entry primitive — short angled stub at `entry.position`
//! extending by `entry.size = (dx, dy)` so a wire endpoint sits on the
//! bus line.
//!
//! Renders with the active theme's `bus` colour at the
//! [`bus::BUS_STROKE_MM`](super::bus::BUS_STROKE_MM) weight. See
//! `signex_types::schematic::BusEntry`.

use iced::widget::canvas::{Frame, Path, Stroke};
use signex_types::schematic::{Aabb, BusEntry, Point, SelectedItem, SelectedKind};

use super::RenderContext;
use super::bus::BUS_STROKE_MM;
use super::util::{aabbs_overlap, iced_color, point_finite};

const SELECTION_WEIGHT_FACTOR: f64 = 1.5;

/// Render a single bus entry into the content layer's frame.
///
/// `entry.size = (dx, dy)` may be negative on either axis to flip the
/// stub direction; all four diagonal orientations render correctly.
pub fn draw_bus_entry(frame: &mut Frame, entry: &BusEntry, ctx: &RenderContext<'_>) {
    let bbox = bus_entry_aabb(entry);
    if !aabbs_overlap(&bbox, &ctx.visible_world_bounds()) {
        return;
    }

    let (a, b) = endpoints(entry);
    let start = ctx.viewport.world_to_screen(a);
    let end = ctx.viewport.world_to_screen(b);
    if !point_finite(start) || !point_finite(end) {
        return;
    }

    let selected = ctx.is_selected(&SelectedItem::new(entry.uuid, SelectedKind::BusEntry));
    let width_world = BUS_STROKE_MM
        * if selected {
            SELECTION_WEIGHT_FACTOR
        } else {
            1.0
        };
    let width_px = (width_world * ctx.viewport.zoom_px_per_mm()).max(1.0) as f32;

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

/// World-space AABB of a bus entry — covers both endpoints.
#[inline]
pub(crate) fn bus_entry_aabb(entry: &BusEntry) -> Aabb {
    let (a, b) = endpoints(entry);
    Aabb::new(a.x, a.y, b.x, b.y)
}

#[inline]
fn endpoints(entry: &BusEntry) -> (Point, Point) {
    let a = entry.position;
    let b = Point::new(a.x + entry.size.0, a.y + entry.size.1);
    (a, b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn test_entry(pos: Point, size: (f64, f64)) -> BusEntry {
        BusEntry {
            uuid: Uuid::new_v4(),
            position: pos,
            size,
        }
    }

    #[test]
    fn aabb_covers_both_endpoints_in_negative_quadrant() {
        // Edge case: dx < 0 and dy < 0 — the stub sloping up-left.
        let e = test_entry(Point::new(0.0, 0.0), (-2.54, -2.54));
        let bbox = bus_entry_aabb(&e);
        assert!(bbox.contains(0.0, 0.0));
        assert!(bbox.contains(-2.54, -2.54));
        assert!(bbox.contains(-1.0, -1.0));
    }

    #[test]
    fn endpoints_offset_by_size() {
        let e = test_entry(Point::new(1.0, 2.0), (3.0, 4.0));
        let (a, b) = endpoints(&e);
        assert_eq!(a.x, 1.0);
        assert_eq!(a.y, 2.0);
        assert_eq!(b.x, 4.0);
        assert_eq!(b.y, 6.0);
    }
}
