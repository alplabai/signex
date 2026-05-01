//! Schematic labels — net, global, hierarchical. Power labels are
//! rendered via their parent symbol in [`super::symbol`] and are
//! intentionally skipped here.
//!
//! Spec: `docs/RENDERING_RULES.md::sch-labels`. Net labels paint plain
//! text bottom-aligned at the wire endpoint; global / hier labels paint
//! a directional 5-sided flag polygon with the text inside, with the
//! point direction derived from `label.rotation` (0° points right,
//! 90° up, 180° left, 270° down). Flag dimensions are derived from
//! rendered text height multiplied by Signex-tuned constants.
//!
//! All glyph layout delegates to [`super::text::draw_rotated_text`].

use iced::widget::canvas::{Frame, Path, Stroke};
use signex_types::schematic::{
    Aabb, HAlign, Label, LabelType, Point, SelectedItem, SelectedKind, VAlign,
};

use super::RenderContext;
use super::text::{DEFAULT_TEXT_MM, draw_rotated_text, mm_to_text_pixels};
use super::util::{aabbs_overlap, iced_color, point_finite};

/// Multiplier on the text height that drives flag-polygon height.
/// Signex-chosen: gives a comfortable margin around the glyphs without
/// crowding the page.
const FLAG_HEIGHT_FACTOR: f64 = 1.6;

/// Multiplier on the text height that drives the flag's pointed-end
/// depth (`< 1.0` so the point is shorter than the body height).
const FLAG_POINT_FACTOR: f64 = 0.8;

/// Multiplier on the text height that drives the flag's left/right
/// padding around the glyph run.
const FLAG_PAD_FACTOR: f64 = 0.4;

/// Approximate glyph aspect ratio used to size the flag width when we
/// don't have iced-side glyph metrics yet.
const GLYPH_ASPECT: f64 = 0.6;

const SELECTION_WEIGHT_FACTOR: f64 = 1.5;

/// **Deprecated v0.12 placement-preview helper.** Paints a single
/// label ghost in caller-chosen stroke + fill colours, without needing
/// a snapshot. v0.12: only the stroke colour is honoured (flag fill
/// is left transparent). Used by signex-app's placement tools.
#[deprecated(since = "0.12.0", note = "build a RenderContext and call draw_label")]
pub fn draw_label_preview(
    frame: &mut Frame,
    label: &Label,
    viewport: &super::Viewport,
    stroke_color: iced::Color,
    _fill_color: iced::Color,
) {
    if label.text.is_empty() || matches!(label.label_type, LabelType::Power) {
        return;
    }
    let pos = viewport.world_to_screen(label.position);
    if !point_finite(pos) {
        return;
    }
    let mm = if label.font_size > 0.0 {
        label.font_size
    } else {
        DEFAULT_TEXT_MM
    };
    let scale = crate::canvas_font_size_scale() as f64;
    let em_mm = mm / 0.72;
    let size_px = (em_mm * viewport.zoom_px_per_mm() * scale) as f32;
    super::text::draw_rotated_text(
        frame,
        &label.text,
        pos,
        label.rotation,
        size_px,
        stroke_color,
        label.justify,
        label.justify_v,
    );
}

/// Render a single label into the content layer's frame. Power labels
/// are silently skipped (the parent symbol owns their visuals).
pub fn draw_label(frame: &mut Frame, label: &Label, ctx: &RenderContext<'_>) {
    if matches!(label.label_type, LabelType::Power) {
        return;
    }
    if label.text.is_empty() {
        return;
    }

    let bbox = label_aabb(label);
    if !aabbs_overlap(&bbox, &ctx.visible_world_bounds()) {
        return;
    }

    let selected = ctx.is_selected(&SelectedItem::new(label.uuid, SelectedKind::Label));
    let text_colour = label_colour(label, selected, ctx);

    match label.label_type {
        LabelType::Net => draw_net_label(frame, label, text_colour, ctx),
        LabelType::Global | LabelType::Hierarchical => {
            draw_flag_label(frame, label, text_colour, selected, ctx)
        }
        LabelType::Power => unreachable!("power label was filtered out above"),
    }
}

fn draw_net_label(frame: &mut Frame, label: &Label, colour: iced::Color, ctx: &RenderContext<'_>) {
    let pos = ctx.viewport.world_to_screen(label.position);
    if !point_finite(pos) {
        return;
    }
    let size_px = mm_to_text_pixels(label.font_size, ctx);
    draw_rotated_text(
        frame,
        &label.text,
        pos,
        label.rotation,
        size_px,
        colour,
        label.justify,
        label.justify_v,
    );
}

fn draw_flag_label(
    frame: &mut Frame,
    label: &Label,
    colour: iced::Color,
    selected: bool,
    ctx: &RenderContext<'_>,
) {
    let text_h_mm = effective_text_mm(label);
    let pad_mm = text_h_mm * FLAG_PAD_FACTOR;
    let body_h_mm = text_h_mm * FLAG_HEIGHT_FACTOR;
    let point_mm = text_h_mm * FLAG_POINT_FACTOR;
    let glyph_w_mm = (text_h_mm * GLYPH_ASPECT) * label.text.chars().count().max(1) as f64;
    let body_w_mm = glyph_w_mm + 2.0 * pad_mm;

    // Build the 5-corner polygon in label-local space (point at +x).
    let half_h = body_h_mm * 0.5;
    let local_corners = [
        Point::new(0.0, 0.0),                      // tip
        Point::new(point_mm, -half_h),             // upper-right of tip
        Point::new(point_mm + body_w_mm, -half_h), // upper-right corner
        Point::new(point_mm + body_w_mm, half_h),  // lower-right corner
        Point::new(point_mm, half_h),              // lower-right of tip
    ];

    // Rotate + translate to world space, then to screen.
    let rad = label.rotation.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();
    let world_corners: Vec<Point> = local_corners
        .iter()
        .map(|p| {
            Point::new(
                label.position.x + p.x * cos - p.y * sin,
                label.position.y + p.x * sin + p.y * cos,
            )
        })
        .collect();

    let path = Path::new(|builder| {
        for (i, p) in world_corners.iter().enumerate() {
            let s = ctx.viewport.world_to_screen(*p);
            if !point_finite(s) {
                return;
            }
            if i == 0 {
                builder.move_to(s);
            } else {
                builder.line_to(s);
            }
        }
        builder.close();
    });

    let stroke_px = (0.15 * ctx.viewport.zoom_px_per_mm()).max(1.0) as f32;
    let stroke_factor = if selected {
        SELECTION_WEIGHT_FACTOR
    } else {
        1.0
    };
    frame.stroke(
        &path,
        Stroke::default()
            .with_width((stroke_px as f64 * stroke_factor) as f32)
            .with_color(colour),
    );

    // Place the text centred inside the body of the flag.
    let body_centre_local = Point::new(point_mm + body_w_mm * 0.5, 0.0);
    let body_centre_world = Point::new(
        label.position.x + body_centre_local.x * cos - body_centre_local.y * sin,
        label.position.y + body_centre_local.x * sin + body_centre_local.y * cos,
    );
    let pos_screen = ctx.viewport.world_to_screen(body_centre_world);
    if !point_finite(pos_screen) {
        return;
    }
    let size_px = mm_to_text_pixels(label.font_size, ctx);
    draw_rotated_text(
        frame,
        &label.text,
        pos_screen,
        label.rotation,
        size_px,
        colour,
        HAlign::Center,
        VAlign::Center,
    );
}

#[inline]
fn effective_text_mm(label: &Label) -> f64 {
    if label.font_size > 0.0 {
        label.font_size
    } else {
        DEFAULT_TEXT_MM
    }
}

fn label_colour(label: &Label, selected: bool, ctx: &RenderContext<'_>) -> iced::Color {
    if selected {
        return iced_color(&ctx.theme().selection);
    }
    match label.label_type {
        LabelType::Net => iced_color(&ctx.theme().net_label),
        LabelType::Global => iced_color(&ctx.theme().global_label),
        LabelType::Hierarchical => iced_color(&ctx.theme().hier_label),
        LabelType::Power => iced_color(&ctx.theme().power),
    }
}

/// Coarse world-space AABB enclosing the label's flag (or its plain
/// text run). Used by frustum culling and Wave 4 hit-test.
pub(crate) fn label_aabb(label: &Label) -> Aabb {
    let text_h = effective_text_mm(label);
    let body_h = text_h * FLAG_HEIGHT_FACTOR;
    let point = text_h * FLAG_POINT_FACTOR;
    let glyph_w = (text_h * GLYPH_ASPECT) * label.text.chars().count().max(1) as f64;
    let body_w = glyph_w + 2.0 * text_h * FLAG_PAD_FACTOR;
    let total_w = match label.label_type {
        LabelType::Net | LabelType::Power => glyph_w,
        LabelType::Global | LabelType::Hierarchical => point + body_w,
    };
    let total_h = body_h.max(text_h);
    let half_h = total_h * 0.5;
    super::text::approx_text_aabb(label.position, total_w, half_h * 2.0, label.rotation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn test_label(label_type: LabelType, rotation: f64, text: &str) -> Label {
        Label {
            uuid: Uuid::new_v4(),
            text: text.to_string(),
            position: Point::new(0.0, 0.0),
            rotation,
            label_type,
            shape: String::new(),
            font_size: 1.27,
            justify: HAlign::Left,
            justify_v: VAlign::Bottom,
        }
    }

    #[test]
    fn net_label_aabb_is_smaller_than_global_label_aabb() {
        let net = label_aabb(&test_label(LabelType::Net, 0.0, "VCC"));
        let global = label_aabb(&test_label(LabelType::Global, 0.0, "VCC"));
        assert!(global.width() > net.width());
    }

    #[test]
    fn hier_and_global_labels_have_same_aabb_shape() {
        let g = label_aabb(&test_label(LabelType::Global, 0.0, "CLK"));
        let h = label_aabb(&test_label(LabelType::Hierarchical, 0.0, "CLK"));
        assert!((g.width() - h.width()).abs() < 1e-9);
        assert!((g.height() - h.height()).abs() < 1e-9);
    }

    #[test]
    fn rotated_label_aabb_grows_orthogonal_axis() {
        // Edge case: a 90°-rotated label's AABB grows in y.
        let upright = label_aabb(&test_label(LabelType::Global, 0.0, "ABCDE"));
        let rotated = label_aabb(&test_label(LabelType::Global, 90.0, "ABCDE"));
        assert!(rotated.height() > upright.height());
    }
}
