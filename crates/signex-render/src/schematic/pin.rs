//! Pin primitive — base stroke + IEEE-Std-91 decorator + name/number text.
//!
//! Pins are drawn as part of a parent symbol's render pass. The parent
//! supplies its [`SymbolTransform`](super::SymbolTransform) so this
//! module can fold the library-space pin geometry into world space.
//!
//! Decorator catalog from `docs/RENDERING_RULES.md::pin-shape-decorators`,
//! which paraphrases IEEE-Std-91 (Graphic Symbols for Logic Functions,
//! IEEE 1984 / 2004 — public industry standard). Decorator dimensions
//! scale with `pin.length` so the marker stays proportional at any zoom
//! and at any user-chosen pin length.
//!
//! Coordinate convention — `pin.position` is the body-side anchor and
//! `pin.length` extends the shaft outward toward the connection point.
//! Decorators that mark the *logical edge* (bubble, clock triangle)
//! sit at the body end; decorators that mark the *connection tip*
//! (output-low slash, hysteresis inputs) sit at the outer end.

use iced::widget::canvas::{Frame, Path, Stroke};
use signex_types::schematic::{Aabb, PIN_NAME_OFFSET_MM, Pin, PinShapeStyle, Point};

use super::text::{draw_rotated_text, mm_to_text_pixels};
use super::util::{aabbs_overlap, iced_color, point_finite};
use super::{RenderContext, SymbolTransform};

/// Default pin shaft stroke width (mm).
pub const PIN_STROKE_MM: f64 = 0.15;

/// Decorator dimensions, all expressed as a multiple of `pin.length`.
/// The IEEE-Std-91 catalog leaves exact proportions to the implementer
/// as long as the markers are unambiguously distinguishable.
const BUBBLE_DIAM_FACTOR: f64 = 0.30;
const TRIANGLE_DEPTH_FACTOR: f64 = 0.40;
const SLASH_HALF_FACTOR: f64 = 0.30;

/// Per-symbol hints that bulk-toggle pin name / number visibility and
/// override the default name offset. Threaded through from
/// [`super::symbol::draw_symbol`] so the parent's `LibSymbol` flags
/// override per-pin defaults — addresses M-2 from the v0.12 review.
#[derive(Debug, Clone, Copy)]
pub struct PinDrawHints {
    pub show_pin_names: bool,
    pub show_pin_numbers: bool,
    pub pin_name_offset_mm: f64,
}

impl Default for PinDrawHints {
    #[inline]
    fn default() -> Self {
        Self {
            show_pin_names: true,
            show_pin_numbers: true,
            pin_name_offset_mm: PIN_NAME_OFFSET_MM,
        }
    }
}

/// Render a single pin into the content layer's frame.
///
/// Hidden pins (`!pin.visible`) early-return; the renderer never paints
/// hidden pins, but [`pin_aabb`] still includes them for hit-testing.
///
/// Calls without per-symbol hints get [`PinDrawHints::default()`] —
/// `show_pin_names = true`, `show_pin_numbers = true`,
/// `pin_name_offset_mm = PIN_NAME_OFFSET_MM`. New code should call
/// [`draw_pin_with_hints`] from the symbol render pass so the parent
/// `LibSymbol`'s visibility toggles win.
pub fn draw_pin(
    frame: &mut Frame,
    pin: &Pin,
    transform: &SymbolTransform,
    ctx: &RenderContext<'_>,
) {
    draw_pin_with_hints(frame, pin, transform, &PinDrawHints::default(), ctx);
}

/// Render a single pin with explicit per-symbol hints. See [`draw_pin`]
/// for the default-hints variant and [`PinDrawHints`] for the field
/// semantics.
pub fn draw_pin_with_hints(
    frame: &mut Frame,
    pin: &Pin,
    transform: &SymbolTransform,
    hints: &PinDrawHints,
    ctx: &RenderContext<'_>,
) {
    if !pin.visible {
        return;
    }

    // Body anchor + connection tip in world space.
    let body_w = transform.apply(pin.position);
    let dir_local = direction_unit(pin.rotation);
    let tip_local = Point::new(
        pin.position.x + pin.length * dir_local.x,
        pin.position.y + pin.length * dir_local.y,
    );
    let tip_w = transform.apply(tip_local);

    // Frustum cull.
    let bbox = pin_aabb(pin, transform);
    if !aabbs_overlap(&bbox, &ctx.visible_world_bounds()) {
        return;
    }

    let body_s = ctx.viewport.world_to_screen(body_w);
    let tip_s = ctx.viewport.world_to_screen(tip_w);
    if !point_finite(body_s) || !point_finite(tip_s) {
        return;
    }

    let pin_colour = iced_color(&ctx.theme().pin);
    let stroke_px = (PIN_STROKE_MM * ctx.viewport.zoom_px_per_mm()).max(1.0) as f32;
    let stroke = Stroke::default()
        .with_width(stroke_px)
        .with_color(pin_colour);

    // Shaft line — body to tip.
    frame.stroke(&Path::line(body_s, tip_s), stroke);

    // Decorator overlay.
    draw_decorator(frame, pin, transform, body_w, tip_w, stroke, ctx);

    // Name + number text.
    draw_pin_text(frame, pin, transform, body_w, tip_w, hints, ctx);
}

/// World-space AABB of a pin, including its decorator and name/number
/// text. Used by frustum culling so the bubble / slash / hysteresis
/// triangle and the text run don't get clipped at the viewport edge,
/// and by hit-test so clicks on the name/number register.
///
/// The bounding box is computed by transforming a small set of
/// extreme points in pin-local space (shaft endpoints, perpendicular
/// half-extents at body and tip, and the name anchor offset) so the
/// result remains correct under any parent rotation / mirror.
pub(crate) fn pin_aabb(pin: &Pin, transform: &SymbolTransform) -> Aabb {
    let dir = direction_unit(pin.rotation);
    let length = pin.length;

    // Perpendicular margin covers the slash decorator (`SLASH_HALF_FACTOR
    // × length`) and a generous text-height for pin names painted
    // beside the shaft.
    let perp_margin = (length * SLASH_HALF_FACTOR).max(signex_types::schematic::SCHEMATIC_TEXT_MM);
    // Axial margin covers the bubble decorator's outer edge plus the
    // pin-name anchor offset — both extend past `pin.position` along
    // the negative-axis direction.
    let axial_margin_body = length * BUBBLE_DIAM_FACTOR
        + signex_types::schematic::PIN_NAME_OFFSET_MM
        + signex_types::schematic::SCHEMATIC_TEXT_MM * 0.6 * pin.name.chars().count().max(1) as f64;

    // Local extreme points (Y-up library space). Perpendicular axis is
    // (-dir.y, dir.x).
    let perp = Point::new(-dir.y, dir.x);
    let body = pin.position;
    let tip_local = Point::new(body.x + length * dir.x, body.y + length * dir.y);
    let body_extended = Point::new(
        body.x - dir.x * axial_margin_body,
        body.y - dir.y * axial_margin_body,
    );
    let candidates_local = [
        body,
        tip_local,
        body_extended,
        // Perpendicular extents at body and tip.
        Point::new(body.x + perp.x * perp_margin, body.y + perp.y * perp_margin),
        Point::new(body.x - perp.x * perp_margin, body.y - perp.y * perp_margin),
        Point::new(
            tip_local.x + perp.x * perp_margin,
            tip_local.y + perp.y * perp_margin,
        ),
        Point::new(
            tip_local.x - perp.x * perp_margin,
            tip_local.y - perp.y * perp_margin,
        ),
    ];

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for p in candidates_local {
        let w = transform.apply(p);
        min_x = min_x.min(w.x);
        min_y = min_y.min(w.y);
        max_x = max_x.max(w.x);
        max_y = max_y.max(w.y);
    }
    Aabb::new(min_x, min_y, max_x, max_y)
}

#[allow(clippy::too_many_arguments)]
fn draw_decorator(
    frame: &mut Frame,
    pin: &Pin,
    transform: &SymbolTransform,
    body_w: Point,
    tip_w: Point,
    stroke: Stroke<'_>,
    ctx: &RenderContext<'_>,
) {
    use PinShapeStyle as PS;
    let _ = tip_w; // kept for future per-decorator placement variants
    match pin.shape_style {
        PS::Plain => {}
        PS::InvertedBubble => bubble_at(frame, body_w, pin.length, ctx, stroke),
        PS::ClockTriangle => triangle_at(
            frame,
            body_w,
            pin.rotation,
            transform,
            pin.length,
            ctx,
            stroke,
            /* outward = */ false,
        ),
        PS::InvertedClockBubble => {
            bubble_at(frame, body_w, pin.length, ctx, stroke);
            triangle_at(
                frame,
                body_w,
                pin.rotation,
                transform,
                pin.length,
                ctx,
                stroke,
                false,
            );
        }
        PS::HysteresisInput => triangle_at(
            frame,
            tip_w,
            pin.rotation,
            transform,
            pin.length,
            ctx,
            stroke,
            /* outward = */ true,
        ),
        PS::HysteresisOutput => slash_at(
            frame,
            tip_w,
            pin.rotation,
            transform,
            pin.length,
            ctx,
            stroke,
        ),
        PS::Schmitt => {
            triangle_at(
                frame,
                body_w,
                pin.rotation,
                transform,
                pin.length,
                ctx,
                stroke,
                false,
            );
            slash_at(
                frame,
                tip_w,
                pin.rotation,
                transform,
                pin.length,
                ctx,
                stroke,
            );
        }
    }
}

fn bubble_at(
    frame: &mut Frame,
    centre_w: Point,
    pin_length_mm: f64,
    ctx: &RenderContext<'_>,
    stroke: Stroke<'_>,
) {
    let radius_mm = pin_length_mm * BUBBLE_DIAM_FACTOR * 0.5;
    let centre_s = ctx.viewport.world_to_screen(centre_w);
    if !point_finite(centre_s) {
        return;
    }
    let r_px = (radius_mm * ctx.viewport.zoom_px_per_mm()).max(1.0) as f32;
    frame.stroke(&Path::circle(centre_s, r_px), stroke);
}

#[allow(clippy::too_many_arguments)]
fn triangle_at(
    frame: &mut Frame,
    apex_w: Point,
    pin_rotation_local: f64,
    transform: &SymbolTransform,
    pin_length_mm: f64,
    ctx: &RenderContext<'_>,
    stroke: Stroke<'_>,
    outward: bool,
) {
    // Triangle apex points along the pin axis. `outward = true` ⇒
    // apex points away from body (out beyond the tip);
    // `outward = false` ⇒ apex points into the body.
    let depth_mm = pin_length_mm * TRIANGLE_DEPTH_FACTOR;

    // Local pin direction (unit vector in library coords).
    let local_dir = direction_unit(pin_rotation_local);
    // Build apex direction in *world* space by transforming a tiny
    // local-direction step and re-extracting the unit vector.
    let local_step = Point::new(
        if outward { local_dir.x } else { -local_dir.x } * depth_mm,
        if outward { local_dir.y } else { -local_dir.y } * depth_mm,
    );
    // Reuse base_dir_anchor_w to derive the world-direction by sampling
    // a step away from the apex along the pin axis.
    let off_local_origin = transform.apply(Point::ZERO);
    let off_local_dir = transform.apply(local_step);
    let world_dir = Point::new(
        off_local_dir.x - off_local_origin.x,
        off_local_dir.y - off_local_origin.y,
    );
    // Perpendicular: rotate world_dir 90° in screen Y-down space.
    let perp = Point::new(-world_dir.y, world_dir.x);
    // Triangle base mid-point is `apex - world_dir`.
    let base_mid = Point::new(apex_w.x - world_dir.x, apex_w.y - world_dir.y);
    let half = depth_mm * 0.5;
    let len_perp = (perp.x * perp.x + perp.y * perp.y).sqrt().max(1e-9);
    let perp_unit = Point::new(perp.x / len_perp, perp.y / len_perp);
    let base_a = Point::new(
        base_mid.x + perp_unit.x * half,
        base_mid.y + perp_unit.y * half,
    );
    let base_b = Point::new(
        base_mid.x - perp_unit.x * half,
        base_mid.y - perp_unit.y * half,
    );

    let apex_s = ctx.viewport.world_to_screen(apex_w);
    let a_s = ctx.viewport.world_to_screen(base_a);
    let b_s = ctx.viewport.world_to_screen(base_b);
    if !point_finite(apex_s) || !point_finite(a_s) || !point_finite(b_s) {
        return;
    }

    let path = Path::new(|builder| {
        builder.move_to(apex_s);
        builder.line_to(a_s);
        builder.line_to(b_s);
        builder.close();
    });
    frame.stroke(&path, stroke);
}

fn slash_at(
    frame: &mut Frame,
    centre_w: Point,
    pin_rotation_local: f64,
    transform: &SymbolTransform,
    pin_length_mm: f64,
    ctx: &RenderContext<'_>,
    stroke: Stroke<'_>,
) {
    // The slash is perpendicular to the pin axis, half-length
    // SLASH_HALF_FACTOR × pin_length, centred at `centre_w`.
    let half_mm = pin_length_mm * SLASH_HALF_FACTOR;
    let local_dir = direction_unit(pin_rotation_local);
    // Perpendicular in local space (rotated 90° CCW in Y-up library).
    let local_perp_step = Point::new(-local_dir.y * half_mm, local_dir.x * half_mm);
    let local_origin = Point::ZERO;
    let world_origin = transform.apply(local_origin);
    let world_perp_end = transform.apply(local_perp_step);
    let world_dir = Point::new(
        world_perp_end.x - world_origin.x,
        world_perp_end.y - world_origin.y,
    );

    let a_w = Point::new(centre_w.x + world_dir.x, centre_w.y + world_dir.y);
    let b_w = Point::new(centre_w.x - world_dir.x, centre_w.y - world_dir.y);
    let a_s = ctx.viewport.world_to_screen(a_w);
    let b_s = ctx.viewport.world_to_screen(b_w);
    if !point_finite(a_s) || !point_finite(b_s) {
        return;
    }
    frame.stroke(&Path::line(a_s, b_s), stroke);
}

fn draw_pin_text(
    frame: &mut Frame,
    pin: &Pin,
    _transform: &SymbolTransform,
    body_w: Point,
    tip_w: Point,
    hints: &PinDrawHints,
    ctx: &RenderContext<'_>,
) {
    let font_mm = signex_types::schematic::SCHEMATIC_TEXT_MM;
    let size_px = mm_to_text_pixels(font_mm, ctx);
    let color = iced_color(&ctx.theme().pin);

    // Reduce the world-space pin direction to one of {right, up, left,
    // down}. We use this to (a) place the name anchor in the correct
    // perpendicular / axial direction after the parent transform, and
    // (b) pick a horizontal-text rotation + justify that reads
    // correctly regardless of how the parent rotates / mirrors.
    let dir_w = Point::new(tip_w.x - body_w.x, tip_w.y - body_w.y);
    let len = (dir_w.x * dir_w.x + dir_w.y * dir_w.y).sqrt().max(1e-9);
    let dx = dir_w.x / len;
    let dy = dir_w.y / len;

    if hints.show_pin_names && pin.name_visible && !pin.name.is_empty() {
        // Name anchor: PIN_NAME_OFFSET_MM along the *opposite* of the
        // pin's world-direction so the text lands inside the body.
        let offset = hints.pin_name_offset_mm.max(0.0);
        let anchor_w = Point::new(body_w.x - dx * offset, body_w.y - dy * offset);
        let anchor_s = ctx.viewport.world_to_screen(anchor_w);
        if point_finite(anchor_s) {
            // Always render horizontally — whichever side of the body the
            // name lands on, choose justify so the text grows away from
            // the body. (Reads top-down for vertical pins via VAlign.)
            let (justify_h, justify_v) = name_justify_for_direction(dx, dy);
            draw_rotated_text(
                frame, &pin.name, anchor_s, 0.0, size_px, color, justify_h, justify_v,
            );
        }
    }

    if hints.show_pin_numbers && pin.number_visible && !pin.number.is_empty() {
        // Number sits along the shaft midpoint, offset perpendicular
        // to the shaft so the digits don't overlap the stroke.
        let mid_w = Point::new((body_w.x + tip_w.x) * 0.5, (body_w.y + tip_w.y) * 0.5);
        let anchor_s = ctx.viewport.world_to_screen(mid_w);
        if point_finite(anchor_s) {
            let (justify_h, justify_v) = number_justify_for_direction(dx, dy);
            draw_rotated_text(
                frame,
                &pin.number,
                anchor_s,
                0.0,
                size_px,
                color,
                justify_h,
                justify_v,
            );
        }
    }
}

/// Pick (HAlign, VAlign) so a horizontal pin-name run grows *away*
/// from the body, given the pin's world-direction unit vector.
fn name_justify_for_direction(
    dx: f64,
    dy: f64,
) -> (
    signex_types::schematic::HAlign,
    signex_types::schematic::VAlign,
) {
    use signex_types::schematic::{HAlign, VAlign};
    if dx.abs() >= dy.abs() {
        if dx >= 0.0 {
            (HAlign::Right, VAlign::Center) // pin →; name to left of body
        } else {
            (HAlign::Left, VAlign::Center) // pin ←; name to right of body
        }
    } else if dy >= 0.0 {
        (HAlign::Center, VAlign::Bottom) // pin ↓ (screen); name above body
    } else {
        (HAlign::Center, VAlign::Top) // pin ↑ (screen); name below body
    }
}

/// Pick (HAlign, VAlign) for the pin number — sits perpendicular to the
/// shaft midpoint, on the "above" side relative to the pin direction.
fn number_justify_for_direction(
    dx: f64,
    dy: f64,
) -> (
    signex_types::schematic::HAlign,
    signex_types::schematic::VAlign,
) {
    use signex_types::schematic::{HAlign, VAlign};
    if dx.abs() >= dy.abs() {
        // Horizontal pin — number above the shaft.
        (HAlign::Center, VAlign::Bottom)
    } else {
        // Vertical pin — number to the right of the shaft.
        (HAlign::Left, VAlign::Center)
    }
}

/// Unit vector for a pin's rotation. `pin.rotation` is in degrees,
/// using the same Y-flipped library convention the symbol transform
/// follows: 0° = +x, 90° = +y (visually down in screen Y-down).
#[inline]
fn direction_unit(rotation_deg: f64) -> Point {
    let rad = rotation_deg.to_radians();
    Point::new(rad.cos(), rad.sin())
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::PinDirection;

    fn test_pin(rotation: f64, length: f64, shape: PinShapeStyle) -> Pin {
        Pin {
            direction: PinDirection::Input,
            shape_style: shape,
            position: Point::new(0.0, 0.0),
            rotation,
            length,
            name: "A".to_string(),
            number: "1".to_string(),
            visible: true,
            name_visible: true,
            number_visible: true,
        }
    }

    fn identity_transform() -> SymbolTransform {
        SymbolTransform {
            origin: Point::new(0.0, 0.0),
            rotation_deg: 0.0,
            mirror_x: false,
            mirror_y: false,
        }
    }

    #[test]
    fn pin_aabb_extends_along_rotation() {
        let p = test_pin(0.0, 2.54, PinShapeStyle::Plain);
        let bbox = pin_aabb(&p, &identity_transform());
        assert!(bbox.width() > 0.0 || bbox.height() > 0.0);
    }

    #[test]
    fn pin_aabb_under_parent_rotation_swaps_axes() {
        let p = test_pin(0.0, 2.54, PinShapeStyle::Plain);
        let mut tx = identity_transform();
        // Parent rotated 90° — the world-space pin should now extend
        // mostly in y rather than x.
        tx.rotation_deg = 90.0;
        let bbox = pin_aabb(&p, &tx);
        assert!(bbox.height() > bbox.width(), "bbox = {:?}", bbox);
    }

    #[test]
    fn direction_unit_is_unit_length_for_canonical_angles() {
        for deg in [0.0, 90.0, 180.0, 270.0] {
            let v = direction_unit(deg);
            let len = (v.x * v.x + v.y * v.y).sqrt();
            assert!((len - 1.0).abs() < 1e-9, "deg={} len={}", deg, len);
        }
    }
}
