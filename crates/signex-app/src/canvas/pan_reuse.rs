//! Moving retained canvas geometry by a pan instead of tessellating it
//! again: the one rule the schematic content (#2078) and the CPU board
//! content (#2321) share. See `docs/RENDERING_RULES.md`, "sch-pan-reuse".

use iced::{Point, Rectangle, Size, Vector};

use crate::schematic_runtime::pan_reach;

/// Tolerance, in device pixels, for a pan to count as a whole number of them.
/// Widened to the f32 resolution of the offsets when they are large (a view
/// zoomed far in), where 1e-3 is below one step of the camera.
const WHOLE_PX_TOLERANCE: f64 = 1e-3;

/// How far the clip rectangle reaches past the viewport on each side: the
/// pan reach cut down to whole device pixels.
pub(crate) fn clip_margin(viewport: Size, device_px_per_logical_px: f32) -> Vector {
    let reach = pan_reach(viewport);
    let whole = |v: f32| (v * device_px_per_logical_px).floor() / device_px_per_logical_px;
    Vector::new(whole(reach.x), whole(reach.y))
}

/// The frame's clip rectangle: the viewport grown by [`clip_margin`].
pub(crate) fn clip_rect(viewport: Size, device_px_per_logical_px: f32) -> Rectangle {
    let margin = clip_margin(viewport, device_px_per_logical_px);
    Rectangle::new(
        Point::new(-margin.x, -margin.y),
        Size::new(
            viewport.width + 2.0 * margin.x,
            viewport.height + 2.0 * margin.y,
        ),
    )
}

/// The translation that carries geometry built at camera offset `from` to
/// offset `to`, or `None` when it is not a whole number of device pixels on
/// each axis (hairline snapping works on that grid and only commutes with
/// whole-pixel shifts) or is longer than `limit` on an axis.
pub(crate) fn whole_pixel_shift(
    from: (f32, f32),
    to: (f32, f32),
    device_px_per_logical_px: f32,
    limit: Vector,
) -> Option<Vector> {
    let dpr = f64::from(device_px_per_logical_px);
    let axis = |from: f32, to: f32, limit: f32| {
        // Exact: the difference of two f32 fits an f64.
        let moved = (f64::from(to) - f64::from(from)) * dpr;
        let whole = moved.round();
        let step = f64::from(f32::EPSILON) * f64::from(to.abs().max(from.abs()));
        let tolerance = WHOLE_PX_TOLERANCE.max(2.0 * step * dpr);
        let shift = (whole / dpr) as f32;
        ((moved - whole).abs() < tolerance && shift.abs() <= limit).then_some(shift)
    };
    Some(Vector::new(
        axis(from.0, to.0, limit.x)?,
        axis(from.1, to.1, limit.y)?,
    ))
}
