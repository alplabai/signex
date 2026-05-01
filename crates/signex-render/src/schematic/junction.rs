//! Junction primitive — filled disc at a wire intersection.
//!
//! Drawn as a single filled circle at `junction.position`. Diameter
//! comes from `junction.diameter` when non-zero, else
//! [`JUNCTION_DEFAULT_DIAMETER_MM`]. Colour is the active theme's
//! `junction` token. See `signex_types::schematic::Junction`.

use iced::widget::canvas::{Frame, Path};
use signex_types::schematic::{Aabb, Junction, SelectedItem, SelectedKind};

use super::RenderContext;
use super::util::{aabbs_overlap, iced_color, point_finite};

/// Default junction diameter — `0.9144 mm` (= 36 mil). Chosen so a
/// junction is unambiguously visible at the typical 100 mil grid
/// without crowding the wire endpoints.
pub const JUNCTION_DEFAULT_DIAMETER_MM: f64 = 0.9144;

/// Render a single junction into the content layer's frame.
pub fn draw_junction(frame: &mut Frame, junction: &Junction, ctx: &RenderContext<'_>) {
    let radius_mm = effective_diameter_mm(junction) * 0.5;
    let bbox = junction_aabb(junction);
    if !aabbs_overlap(&bbox, &ctx.viewport.visible_world_bounds()) {
        return;
    }

    let centre = ctx.viewport.world_to_screen(junction.position);
    if !point_finite(centre) {
        return;
    }
    let radius_px = (radius_mm * ctx.viewport.zoom_px_per_mm).max(1.0) as f32;

    let selected = ctx.is_selected(&SelectedItem::new(junction.uuid, SelectedKind::Junction));
    let colour = if selected {
        iced_color(&ctx.theme().selection)
    } else {
        iced_color(&ctx.theme().junction)
    };

    frame.fill(&Path::circle(centre, radius_px), colour);
}

/// World-space AABB enclosing the junction disc — `pub(crate)` for
/// hit-test reuse.
#[inline]
pub(crate) fn junction_aabb(junction: &Junction) -> Aabb {
    let r = effective_diameter_mm(junction) * 0.5;
    Aabb::new(
        junction.position.x - r,
        junction.position.y - r,
        junction.position.x + r,
        junction.position.y + r,
    )
}

/// Apply the schematic default when the junction's stored diameter is
/// zero or negative.
#[inline]
pub(crate) fn effective_diameter_mm(junction: &Junction) -> f64 {
    if junction.diameter > 0.0 {
        junction.diameter
    } else {
        JUNCTION_DEFAULT_DIAMETER_MM
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::Point;
    use uuid::Uuid;

    fn test_junction(pos: Point, diameter: f64) -> Junction {
        Junction {
            uuid: Uuid::new_v4(),
            position: pos,
            diameter,
        }
    }

    #[test]
    fn default_diameter_used_when_zero() {
        let j = test_junction(Point::ZERO, 0.0);
        assert!((effective_diameter_mm(&j) - JUNCTION_DEFAULT_DIAMETER_MM).abs() < f64::EPSILON);
    }

    #[test]
    fn aabb_centred_on_position_with_correct_radius() {
        let j = test_junction(Point::new(2.0, 3.0), 1.0);
        let bbox = junction_aabb(&j);
        assert!((bbox.min_x - 1.5).abs() < 1e-9);
        assert!((bbox.max_x - 2.5).abs() < 1e-9);
        assert!((bbox.min_y - 2.5).abs() < 1e-9);
        assert!((bbox.max_y - 3.5).abs() < 1e-9);
    }

    #[test]
    fn negative_diameter_falls_back_to_default() {
        // Edge case: corrupted .snxsch could surface a negative; the
        // default keeps the renderer non-degenerate.
        let j = test_junction(Point::ZERO, -0.5);
        assert!(effective_diameter_mm(&j) > 0.0);
    }
}
