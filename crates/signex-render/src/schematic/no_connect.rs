//! No-connect primitive — small "X" marker at an unconnected pin.
//!
//! Two short crossing strokes centred on `nc.position`, drawn in the
//! active theme's `no_connect` colour. Marker dimensions match
//! standard EDA convention. See `signex_types::schematic::NoConnect`.

use iced::widget::canvas::{Frame, Path, Stroke};
use signex_types::schematic::{Aabb, NoConnect, Point, SelectedItem, SelectedKind};

use super::RenderContext;
use super::util::{aabbs_overlap, iced_color, point_finite};

/// Half-length of each leg of the X marker. World mm.
pub const NO_CONNECT_HALF_SIZE_MM: f64 = 0.6;

/// Stroke width used by the X marker. World mm.
pub const NO_CONNECT_STROKE_MM: f64 = 0.15;

const SELECTION_WEIGHT_FACTOR: f64 = 1.5;

/// Render a single no-connect marker into the content layer's frame.
pub fn draw_no_connect(frame: &mut Frame, nc: &NoConnect, ctx: &RenderContext<'_>) {
    let bbox = no_connect_aabb(nc);
    if !aabbs_overlap(&bbox, &ctx.viewport.visible_world_bounds()) {
        return;
    }

    let h = NO_CONNECT_HALF_SIZE_MM;
    let world_pts = [
        // Stroke 1: top-left → bottom-right.
        (
            Point::new(nc.position.x - h, nc.position.y - h),
            Point::new(nc.position.x + h, nc.position.y + h),
        ),
        // Stroke 2: top-right → bottom-left.
        (
            Point::new(nc.position.x + h, nc.position.y - h),
            Point::new(nc.position.x - h, nc.position.y + h),
        ),
    ];

    let selected = ctx.is_selected(&SelectedItem::new(nc.uuid, SelectedKind::NoConnect));
    let width_world = NO_CONNECT_STROKE_MM
        * if selected {
            SELECTION_WEIGHT_FACTOR
        } else {
            1.0
        };
    let width_px = (width_world * ctx.viewport.zoom_px_per_mm).max(1.0) as f32;

    let colour = if selected {
        iced_color(&ctx.theme().selection)
    } else {
        iced_color(&ctx.theme().no_connect)
    };

    let stroke = Stroke::default().with_width(width_px).with_color(colour);

    for (a, b) in world_pts {
        let sa = ctx.viewport.world_to_screen(a);
        let sb = ctx.viewport.world_to_screen(b);
        if point_finite(sa) && point_finite(sb) {
            frame.stroke(&Path::line(sa, sb), stroke);
        }
    }
}

/// World-space AABB enclosing the X marker.
#[inline]
pub(crate) fn no_connect_aabb(nc: &NoConnect) -> Aabb {
    let h = NO_CONNECT_HALF_SIZE_MM;
    Aabb::new(
        nc.position.x - h,
        nc.position.y - h,
        nc.position.x + h,
        nc.position.y + h,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn test_nc(pos: Point) -> NoConnect {
        NoConnect {
            uuid: Uuid::new_v4(),
            position: pos,
        }
    }

    #[test]
    fn aabb_centred_on_position_with_marker_half_size() {
        let nc = test_nc(Point::new(0.0, 0.0));
        let bbox = no_connect_aabb(&nc);
        assert!((bbox.min_x - (-NO_CONNECT_HALF_SIZE_MM)).abs() < 1e-9);
        assert!((bbox.max_x - NO_CONNECT_HALF_SIZE_MM).abs() < 1e-9);
        assert_eq!(bbox.width(), 2.0 * NO_CONNECT_HALF_SIZE_MM);
    }

    #[test]
    fn aabb_includes_neighbouring_marker_when_close() {
        // Edge case: two markers within their half-size of each other
        // produce overlapping AABBs — the renderer must still draw both.
        let a = no_connect_aabb(&test_nc(Point::new(0.0, 0.0)));
        let b = no_connect_aabb(&test_nc(Point::new(NO_CONNECT_HALF_SIZE_MM, 0.0)));
        assert!(super::super::util::aabbs_overlap(&a, &b));
    }
}
