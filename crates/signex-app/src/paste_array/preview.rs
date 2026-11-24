//! The Paste Array preview (#2527): a read-only line-art drawing of where the
//! copies land, on the sheet grid, the original in the accent, with X / Y
//! spacing callouts. It can be drawn taller; no drag, no on-image edit: the
//! fields stay the only place to edit.

use iced::{Point, Rectangle};
use signex_i18n::UserText;
use signex_types::text_increment::increment_trailing_number;
use signex_widgets::property_illustration::{
    Anchor, Callout, FieldId, Illustration, Ink, Paint, Part, Primitive, Style, Text, TextLabel,
    TextRole, Tint, Weight,
};

/// Most copies the preview draws, the original included.
pub const PREVIEW_MAX_DRAWN: usize = 12;

/// The X spacing field and its callout.
pub const FIELD_X: FieldId = FieldId::new("paste-array-x");
/// The Y spacing field and its callout.
pub const FIELD_Y: FieldId = FieldId::new("paste-array-y");

/// The copy size drawn when the copy has no measurable extent, in mm.
const FALLBACK_COPY_MM: f64 = 5.08;
/// Grid pitch drawn when the sheet has none, in mm.
const FALLBACK_GRID_MM: f64 = 2.54;
/// Most grid lines drawn across the preview; the pitch doubles until it fits.
const MAX_GRID_LINES: f64 = 40.0;
/// Margin around the copies, as a share of the larger extent.
const MARGIN_SHARE: f64 = 0.15;
/// Height of the drawing in captions, as it opens and expanded.
const PREVIEW_EM: f32 = 16.0;
const PREVIEW_EXPANDED_EM: f32 = 30.0;

/// The drawing's height in captions.
pub fn height_em(expanded: bool) -> f32 {
    if expanded {
        PREVIEW_EXPANDED_EM
    } else {
        PREVIEW_EM
    }
}

/// How many positions the preview draws, and how many it only counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviewPlan {
    /// Drawn positions, copy 1 (the original) first.
    pub drawn: usize,
    /// Positions past [`PREVIEW_MAX_DRAWN`], named "+N more".
    pub more: usize,
}

/// The plan for `positions` positions, the original included, the first
/// [`PREVIEW_MAX_DRAWN`] drawn.
pub fn preview_plan(positions: u32) -> PreviewPlan {
    let total = positions as usize;
    let drawn = total.min(PREVIEW_MAX_DRAWN);
    PreviewPlan {
        drawn,
        more: total - drawn,
    }
}

/// The label texts of the first `n` copies after the original, counted up by
/// `step` per copy (`D0` by 1 gives `D1, D2, D3`).
pub fn stepped_labels(label: &str, step: i32, n: usize) -> Vec<String> {
    (1..=n as i64)
        .map(|k| increment_trailing_number(label, k * i64::from(step)))
        .collect()
}

/// What the preview draws, in mm on the sheet.
pub struct PreviewSpec {
    /// Positions in the array, the original included.
    pub positions: u32,
    pub dx_mm: f64,
    pub dy_mm: f64,
    pub step: i32,
    pub copy_size_mm: Option<(f64, f64)>,
    pub grid_mm: f64,
    pub sample_label: Option<String>,
    /// The callout texts, already rendered by the view.
    pub x_label: Text,
    pub y_label: Text,
    pub more_label: Text,
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x as f32, y as f32)
}

/// The preview as an illustration, in view units of mm shifted so the
/// drawing starts at the origin.
pub fn illustration(spec: PreviewSpec) -> Illustration {
    let plan = preview_plan(spec.positions);
    let (w, h) = spec
        .copy_size_mm
        .filter(|(w, h)| w.is_finite() && h.is_finite() && *w > 0.0 && *h > 0.0)
        .unwrap_or((FALLBACK_COPY_MM, FALLBACK_COPY_MM));
    let last = (plan.drawn - 1) as f64;
    let (min_x, max_x) = (
        (last * spec.dx_mm).min(0.0),
        (last * spec.dx_mm).max(0.0) + w,
    );
    let (min_y, max_y) = (
        (last * spec.dy_mm).min(0.0),
        (last * spec.dy_mm).max(0.0) + h,
    );
    let margin = (max_x - min_x).max(max_y - min_y) * MARGIN_SHARE + w.max(h) * 0.5;
    let (ox, oy) = (margin - min_x, margin - min_y);
    let view_w = max_x - min_x + 2.0 * margin;
    let view_h = max_y - min_y + 2.0 * margin;

    let mut parts = grid(spec.grid_mm, ox, oy, view_w, view_h);
    for k in 0..plan.drawn {
        let (x, y) = (ox + k as f64 * spec.dx_mm, oy + k as f64 * spec.dy_mm);
        parts.extend(copy_parts(&spec, k, (x, y), (w, h)));
    }
    if plan.more > 0 {
        let (x, y) = (ox + last * spec.dx_mm + w, oy + last * spec.dy_mm + h / 2.0);
        parts.push(Part::label(
            TextLabel {
                at: pt(x + w * 0.2, y),
                text: spec.more_label.clone(),
                anchor: Anchor::Start,
                role: TextRole::Note,
            },
            Ink::Faint,
        ));
    }
    if plan.drawn >= 2 {
        parts.extend(callouts(&spec, (ox, oy), w, margin));
    }
    Illustration::new(view_w as f32, view_h as f32).parts(parts)
}

/// Grid lines at the sheet pitch, doubled until they fit.
pub(crate) fn grid(grid_mm: f64, ox: f64, oy: f64, view_w: f64, view_h: f64) -> Vec<Part> {
    let mut pitch = if grid_mm.is_finite() && grid_mm > 0.0 {
        grid_mm
    } else {
        FALLBACK_GRID_MM
    };
    while view_w.max(view_h) / pitch > MAX_GRID_LINES {
        pitch *= 2.0;
    }
    let rule = Style::outline(Ink::Rule, Weight::Hairline);
    let mut parts = Vec::new();
    let mut x = ox.rem_euclid(pitch);
    while x <= view_w {
        let line = Primitive::Line {
            from: pt(x, 0.0),
            to: pt(x, view_h),
        };
        parts.push(Part::new(line, rule));
        x += pitch;
    }
    let mut y = oy.rem_euclid(pitch);
    while y <= view_h {
        let line = Primitive::Line {
            from: pt(0.0, y),
            to: pt(view_w, y),
        };
        parts.push(Part::new(line, rule));
        y += pitch;
    }
    parts
}

/// Copy `k` (0 is the original): its outline and its label text.
fn copy_parts(spec: &PreviewSpec, k: usize, (x, y): (f64, f64), (w, h): (f64, f64)) -> Vec<Part> {
    let original = k == 0;
    let (ink, tint) = if original {
        (Ink::Accent, Tint::Light)
    } else {
        (Ink::Secondary, Tint::Wash)
    };
    let body = Primitive::RoundedRect {
        rect: Rectangle::new(pt(x, y), iced::Size::new(w as f32, h as f32)),
        radius: (w.min(h) * 0.1) as f32,
    };
    let style = Style::outline(ink, Weight::Regular).with_fill(Paint::Ink(ink), tint);
    let mut parts = vec![Part::new(body, style)];
    if let Some(label) = spec.sample_label.as_deref() {
        let steps = k as i64 * i64::from(spec.step);
        parts.push(Part::label(
            TextLabel {
                at: pt(x + w / 2.0, y + h / 2.0),
                text: UserText::new(increment_trailing_number(label, steps)).into(),
                anchor: Anchor::Middle,
                role: TextRole::Mono,
            },
            ink,
        ));
    }
    parts
}

/// The X spacing above copies 1 and 2, the Y spacing right of them.
fn callouts(spec: &PreviewSpec, (ox, oy): (f64, f64), w: f64, margin: f64) -> Vec<Part> {
    let mut parts = Vec::new();
    let gap = margin * 0.5;
    let (x1, y1) = (ox + spec.dx_mm, oy + spec.dy_mm);
    if spec.dx_mm != 0.0 {
        let line_y = oy.min(y1) - gap;
        parts.push(
            Part::callout(
                Callout::new(pt(ox, line_y), pt(x1, line_y))
                    .label(spec.x_label.clone())
                    .extension(pt(ox, oy), pt(ox, line_y))
                    .extension(pt(x1, y1), pt(x1, line_y)),
            )
            .linked(FIELD_X),
        );
    }
    if spec.dy_mm != 0.0 {
        let line_x = (ox + w).max(x1 + w) + gap;
        parts.push(
            Part::callout(
                Callout::new(pt(line_x, oy), pt(line_x, y1))
                    .label(spec.y_label.clone())
                    .extension(pt(ox + w, oy), pt(line_x, oy))
                    .extension(pt(x1 + w, y1), pt(line_x, y1)),
            )
            .linked(FIELD_Y),
        );
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(positions: u32) -> PreviewSpec {
        PreviewSpec {
            positions,
            dx_mm: 10.16,
            dy_mm: 0.0,
            step: 1,
            copy_size_mm: Some((8.0, 4.0)),
            grid_mm: 1.27,
            sample_label: Some("D0".into()),
            x_label: UserText::new("X 10.16 mm".to_owned()).into(),
            y_label: UserText::new("Y 0 mm".to_owned()).into(),
            more_label: UserText::new("+N more".to_owned()).into(),
        }
    }

    fn bodies(ill: &Illustration) -> usize {
        ill.visible_parts()
            .filter(|p| matches!(p.primitive, Primitive::RoundedRect { .. }))
            .count()
    }

    fn mores(ill: &Illustration) -> usize {
        ill.visible_parts()
            .filter(|p| match &p.primitive {
                Primitive::Label(l) => l.text.as_str() == "+N more",
                _ => false,
            })
            .count()
    }

    #[test]
    fn the_preview_draws_up_to_12_copies_then_counts_the_rest() {
        assert_eq!(preview_plan(4), PreviewPlan { drawn: 4, more: 0 });
        assert_eq!(preview_plan(12), PreviewPlan { drawn: 12, more: 0 });
        assert_eq!(preview_plan(13), PreviewPlan { drawn: 12, more: 1 });
        assert_eq!(
            preview_plan(1000),
            PreviewPlan {
                drawn: 12,
                more: 988
            }
        );

        let small = illustration(spec(4));
        assert_eq!(bodies(&small), 4, "the original and three copies");
        assert_eq!(mores(&small), 0);
        let big = illustration(spec(40));
        assert_eq!(bodies(&big), PREVIEW_MAX_DRAWN);
        assert_eq!(mores(&big), 1, "one '+N more' note");
    }

    #[test]
    fn the_original_is_drawn_in_the_accent_and_spacing_has_a_callout() {
        let ill = illustration(spec(2));
        let accents = ill
            .visible_parts()
            .filter(|p| matches!(p.primitive, Primitive::RoundedRect { .. }))
            .filter(|p| p.style.stroke.map(|s| s.paint) == Some(Paint::Ink(Ink::Accent)))
            .count();
        assert_eq!(accents, 1, "only the original");
        let x = ill
            .visible_parts()
            .filter(|p| p.is_linked_to(&FIELD_X))
            .count();
        let y = ill
            .visible_parts()
            .filter(|p| p.is_linked_to(&FIELD_Y))
            .count();
        assert_eq!((x, y), (1, 0), "no Y callout for a 0 Y spacing");
    }

    /// Mockup row 69: the expand button draws the preview taller.
    #[test]
    fn the_expanded_preview_is_taller() {
        assert!(height_em(true) > height_em(false));
    }

    #[test]
    fn copy_labels_count_up_by_the_step() {
        assert_eq!(stepped_labels("D0", 1, 3), ["D1", "D2", "D3"]);
        assert_eq!(stepped_labels("D0", 2, 2), ["D2", "D4"]);
        assert_eq!(stepped_labels("D0", 0, 2), ["D0", "D0"]);
    }
}
