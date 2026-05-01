//! Free text-note rendering plus shared text-drawing helpers used by
//! [`label`](super::label), [`field_style`](super::field_style),
//! [`symbol`](super::symbol), and [`drawing`](super::drawing).
//!
//! Text size storage is in millimetres on the spec side; iced's canvas
//! takes pixels, so the conversion uses [`crate::SCHEMATIC_TEXT_EM_MM`]
//! (cap-height ≈ 72% of em). Rotation is applied via a save/translate/
//! rotate frame stack so [`iced::widget::canvas::Text`] (which has no
//! rotation field) still ends up oriented correctly.

use iced::Vector;
use iced::advanced::text as advanced_text;
use iced::alignment;
use iced::widget::canvas::{Frame, Text};
use signex_types::schematic::{HAlign, Point, SelectedItem, SelectedKind, TextNote, VAlign};

use super::RenderContext;
use super::util::{iced_color, point_finite};

/// Default schematic text size (mm) when a note's `font_size` is `0.0`.
/// Matches `signex_types::schematic::SCHEMATIC_TEXT_MM`.
pub const DEFAULT_TEXT_MM: f64 = signex_types::schematic::SCHEMATIC_TEXT_MM;

// ---------------------------------------------------------------------------
// v0.11 → v0.12 compatibility shims (pass-through identity).
//
// v0.11 had an `expand_char_escapes` helper that converted backslash-
// style escape sequences (`~FOO~` → overbar markup) into iced-friendly
// glyph runs, and a matching `escape_for_standard` that round-tripped
// the user's typed text back to the storage form. The new renderer
// will eventually take these on through a Signex-original markup spec
// (post-v0.12); for now both are identity functions so consumer code
// keeps compiling. Field text round-trips visually unchanged.
// ---------------------------------------------------------------------------

/// **Deprecated v0.12 shim.** Pass-through; previously decoded
/// backslash escapes for display.
#[deprecated(
    since = "0.12.0",
    note = "v0.13 will replace this with a Signex markup spec; identity for now"
)]
pub fn expand_char_escapes(text: &str) -> String {
    text.to_string()
}

/// **Deprecated v0.12 shim.** Pass-through; previously encoded
/// display text back to storage form.
#[deprecated(
    since = "0.12.0",
    note = "v0.13 will replace this with a Signex markup spec; identity for now"
)]
pub fn escape_for_standard(text: &str) -> String {
    text.to_string()
}

/// Render a single free text note. Hidden notes early-return.
pub fn draw_text_note(frame: &mut Frame, note: &TextNote, ctx: &RenderContext<'_>) {
    if note.text.is_empty() {
        return;
    }
    let pos = ctx.viewport.world_to_screen(note.position);
    if !point_finite(pos) {
        return;
    }

    let selected = ctx.is_selected(&SelectedItem::new(note.uuid, SelectedKind::TextNote));
    let colour = if selected {
        iced_color(&ctx.theme().selection)
    } else {
        // Free text notes use the body / value-text colour token.
        iced_color(&ctx.theme().value)
    };

    let size_px = mm_to_text_pixels(note.font_size, ctx);

    draw_rotated_text(
        frame,
        &note.text,
        pos,
        note.rotation,
        size_px,
        colour,
        note.justify_h,
        note.justify_v,
    );
}

/// Convert a stored field font size (mm) to iced canvas Text pixels.
/// Uses `SCHEMATIC_TEXT_EM_MM` so the rendered cap-height stays
/// consistent with stored mm values.
#[inline]
pub(crate) fn mm_to_text_pixels(stored_mm: f64, ctx: &RenderContext<'_>) -> f32 {
    let mm = if stored_mm > 0.0 {
        stored_mm
    } else {
        DEFAULT_TEXT_MM
    };
    let scale = crate::canvas_font_size_scale() as f64;
    let em_mm = mm / 0.72;
    (em_mm * ctx.viewport.zoom_px_per_mm * scale) as f32
}

/// Draw a single line of text with rotation + alignment, anchored at
/// `pos_screen`. Used by labels, fields, drawing-text, and symbol body
/// text — everything that needs a rotated, justified glyph run.
///
/// The argument list is wide because canvas text mechanics genuinely
/// need every field (content, position, rotation, size, colour, two
/// alignments). Squeezing them into a struct here would turn into
/// builder boilerplate at every call site without making the helper
/// any clearer, so the lint is allowed locally.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_rotated_text(
    frame: &mut Frame,
    text: &str,
    pos_screen: iced::Point,
    rotation_deg: f64,
    size_px: f32,
    color: iced::Color,
    justify_h: HAlign,
    justify_v: VAlign,
) {
    if text.is_empty() || size_px <= 0.0 {
        return;
    }

    let align_x = match justify_h {
        HAlign::Left => advanced_text::Alignment::Left,
        HAlign::Center => advanced_text::Alignment::Center,
        HAlign::Right => advanced_text::Alignment::Right,
    };
    let align_y = match justify_v {
        VAlign::Top => alignment::Vertical::Top,
        VAlign::Center => alignment::Vertical::Center,
        VAlign::Bottom => alignment::Vertical::Bottom,
    };

    let canvas_text = Text {
        content: text.to_string(),
        position: iced::Point::ORIGIN,
        color,
        size: iced::Pixels(size_px),
        font: crate::canvas_font(),
        align_x,
        align_y,
        ..Text::default()
    };

    let radians = rotation_deg.to_radians() as f32;
    if radians.abs() < f32::EPSILON {
        let mut t = canvas_text;
        t.position = pos_screen;
        frame.fill_text(t);
        return;
    }

    frame.with_save(|frame| {
        frame.translate(Vector::new(pos_screen.x, pos_screen.y));
        frame.rotate(iced::Radians(radians));
        frame.fill_text(canvas_text);
    });
}

/// Approximate axis-aligned bounding box for a free text note in
/// world coordinates. Used by hit-test and selection-overlay; we
/// approximate the glyph extent as `(text_len * 0.6 em, 1.0 em)` since
/// canvas-side metrics aren't available before tessellation.
///
/// `#[allow(dead_code)]` for now because Wave 4 hit-test is the only
/// caller and it lands in a follow-on commit.
#[allow(dead_code)]
#[inline]
pub(crate) fn text_note_aabb(note: &TextNote) -> signex_types::schematic::Aabb {
    let mm = if note.font_size > 0.0 {
        note.font_size
    } else {
        DEFAULT_TEXT_MM
    };
    let glyph_w_mm = 0.6 * mm * note.text.chars().count().max(1) as f64;
    let glyph_h_mm = mm;
    approx_text_aabb(note.position, glyph_w_mm, glyph_h_mm, note.rotation)
}

/// Build a coarse text bounding box in world space for a glyph run of
/// `(width_mm × height_mm)` anchored at `pos`. Rotation is folded by
/// rotating the four corners of the unrotated rect.
///
/// `#[allow(dead_code)]` until Wave 4 hit-test wires it in.
#[allow(dead_code)]
pub(crate) fn approx_text_aabb(
    pos: Point,
    width_mm: f64,
    height_mm: f64,
    rotation_deg: f64,
) -> signex_types::schematic::Aabb {
    let rad = -rotation_deg.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();
    let corners = [
        (0.0, 0.0),
        (width_mm, 0.0),
        (0.0, height_mm),
        (width_mm, height_mm),
    ];
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (lx, ly) in corners {
        let wx = pos.x + lx * cos - ly * sin;
        let wy = pos.y + lx * sin + ly * cos;
        min_x = min_x.min(wx);
        max_x = max_x.max(wx);
        min_y = min_y.min(wy);
        max_y = max_y.max(wy);
    }
    signex_types::schematic::Aabb::new(min_x, min_y, max_x, max_y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn test_note(text: &str) -> TextNote {
        TextNote {
            uuid: Uuid::new_v4(),
            text: text.to_string(),
            position: Point::new(5.0, 5.0),
            rotation: 0.0,
            font_size: 0.0,
            justify_h: HAlign::Left,
            justify_v: VAlign::Bottom,
        }
    }

    #[test]
    fn empty_text_aabb_is_minimal_height_box() {
        let note = test_note("");
        let bbox = text_note_aabb(&note);
        assert!(bbox.height() > 0.0);
    }

    #[test]
    fn longer_text_widens_aabb() {
        let short = text_note_aabb(&test_note("AB"));
        let long = text_note_aabb(&test_note("ABCDEFGHIJ"));
        assert!(long.width() > short.width());
    }

    #[test]
    fn rotated_text_aabb_grows_orthogonal_axis() {
        // Edge case: a rotated text run's AABB grows in both directions
        // because the rotated rect's corners lie outside the unrotated
        // span.
        let mut note = test_note("ABCDE");
        let upright = text_note_aabb(&note);
        note.rotation = 30.0;
        let rotated = text_note_aabb(&note);
        assert!(rotated.height() > upright.height());
    }
}
