//! The Paste Array preview (#2527, mockup row 69): a line-art drawing of
//! where the copies land, on the sheet grid, the original in the accent, with
//! X / Y spacing callouts and the copies and step values. Dragging copy 2 sets
//! the spacing; a click on a drawn value edits it on the drawing
//! (`pointer.rs`). The fields stay in sync: both edit the same text.

use iced::{Point, Rectangle};
use signex_i18n::UserText;
use signex_types::text_increment::increment_trailing_number;
use signex_widgets::property_illustration::{
    Anchor, Callout, FieldId, Fit, Illustration, Ink, Paint, Part, Primitive, Style, Text,
    TextLabel, TextRole, Tint, Weight,
};

use super::PasteArrayField;

/// Most copies the preview draws, the original included.
pub const PREVIEW_MAX_DRAWN: usize = 12;

/// The X spacing field and its callout.
pub const FIELD_X: FieldId = FieldId::new("paste-array-x");
/// The Y spacing field and its callout.
pub const FIELD_Y: FieldId = FieldId::new("paste-array-y");

/// Dragging copy 2 steps the spacing by this much, in mm.
pub const DRAG_SNAP_MM: f64 = 1.27;
/// A drag sets no spacing further than this either way, in mm.
pub const DRAG_REACH_MM: f64 = 254.0;

/// The copy size drawn when the copy has no measurable extent, in mm.
const FALLBACK_COPY_MM: f64 = 5.08;
/// Grid pitch drawn when the sheet has none, in mm.
const FALLBACK_GRID_MM: f64 = 2.54;
/// Most grid lines drawn across the preview; the pitch doubles until it fits.
const MAX_GRID_LINES: f64 = 40.0;
/// Margin around the copies, as a share of the larger extent.
const MARGIN_SHARE: f64 = 0.15;
/// The bottom row of values sits this share of the margin above the edge.
const FOOT_SHARE: f64 = 0.3;
/// The bottom row keeps this share of the margin from the side edges.
const EDGE_SHARE: f64 = 0.2;
/// Height of the drawing in captions, as it opens and expanded.
const PREVIEW_EM: f32 = 16.0;
const PREVIEW_EXPANDED_EM: f32 = 30.0;
/// A drawn value takes clicks this many captions wide (from its anchor).
const VALUE_HIT_EM: f32 = 8.0;
/// ... from this many captions above its anchor to this many below.
const VALUE_HIT_ABOVE_EM: f32 = 1.6;
const VALUE_HIT_BELOW_EM: f32 = 0.8;
/// Copy 2 takes a press this many captions past its outline.
const GRAB_PAD_EM: f32 = 0.5;

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
    /// The Copies value drawn on the drawing ("4 copies").
    pub copies_label: Text,
    /// The step value drawn under copy 2 ("+1 per copy"); `None` while label
    /// numbers are not counted up.
    pub step_label: Option<Text>,
}

/// A value drawn on the preview that a click edits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawnValue {
    pub field: PasteArrayField,
    /// Where its label is anchored, in view units.
    pub at: Point,
    pub anchor: Anchor,
}

/// Where the preview puts things, in view units: mm on the sheet, shifted so
/// the drawing starts at the origin.
#[derive(Debug, Clone, PartialEq)]
pub struct PreviewLayout {
    pub view_w: f64,
    pub view_h: f64,
    /// The original's corner.
    pub origin: (f64, f64),
    /// One copy's width and height.
    pub copy: (f64, f64),
    /// The X and Y spacing drawn.
    pub spacing: (f64, f64),
    pub plan: PreviewPlan,
    pub margin: f64,
    /// Every value drawn on the preview, where a click edits it.
    pub values: Vec<DrawnValue>,
}

impl PreviewLayout {
    pub fn view_size(&self) -> iced::Size {
        iced::Size::new(self.view_w as f32, self.view_h as f32)
    }

    /// Copy 2's outline, when it is drawn.
    fn copy_two(&self) -> Option<Rectangle> {
        (self.plan.drawn >= 2).then(|| {
            let (x, y) = (
                self.origin.0 + self.spacing.0,
                self.origin.1 + self.spacing.1,
            );
            Rectangle::new(
                pt(x, y),
                iced::Size::new(self.copy.0 as f32, self.copy.1 as f32),
            )
        })
    }

    /// The drawn value whose label is under `screen`, in a drawing fitted by
    /// `fit` with captions of `text_size` pixels.
    pub fn value_at(&self, fit: &Fit, text_size: f32, screen: Point) -> Option<PasteArrayField> {
        let wide = text_size * VALUE_HIT_EM;
        self.values
            .iter()
            .find(|value| {
                let at = fit.point(value.at);
                let left = match value.anchor {
                    Anchor::Start => at.x,
                    Anchor::Middle => at.x - wide / 2.0,
                    Anchor::End => at.x - wide,
                };
                let top = at.y - text_size * VALUE_HIT_ABOVE_EM;
                let tall = text_size * (VALUE_HIT_ABOVE_EM + VALUE_HIT_BELOW_EM);
                Rectangle::new(Point::new(left, top), iced::Size::new(wide, tall)).contains(screen)
            })
            .map(|value| value.field)
    }

    /// Where the label of `field` is drawn, on screen.
    pub fn value_point(&self, fit: &Fit, field: PasteArrayField) -> Option<(Point, Anchor)> {
        self.values
            .iter()
            .find(|value| value.field == field)
            .map(|value| (fit.point(value.at), value.anchor))
    }

    /// Whether `screen` is on copy 2, the copy a drag moves.
    pub fn is_on_copy_two(&self, fit: &Fit, text_size: f32, screen: Point) -> bool {
        self.copy_two().is_some_and(|rect| {
            let rect = fit.rect(rect);
            let pad = text_size * GRAB_PAD_EM;
            Rectangle::new(
                Point::new(rect.x - pad, rect.y - pad),
                iced::Size::new(rect.width + 2.0 * pad, rect.height + 2.0 * pad),
            )
            .contains(screen)
        })
    }
}

/// Copy 2 held on the preview.
///
/// The drawing refits as the spacing grows, so a move read through the new
/// fit would feed the spacing back into itself. Every move reads through
/// the scale of the press instead.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreviewDrag {
    /// The press, in pixels in the drawing.
    pub press: (f64, f64),
    /// Pixels per mm of the drawing at the press.
    pub scale: f64,
    /// The spacing at the press, in mm.
    pub start: (f64, f64),
}

/// The spacing with the held copy 2 moved to `at`, in pixels in the drawing:
/// the spacing at the press plus the move, snapped to [`DRAG_SNAP_MM`] and
/// kept within [`DRAG_REACH_MM`] either way.
pub fn dragged_spacing(drag: PreviewDrag, at: (f64, f64)) -> (f64, f64) {
    let scale = drag.scale.max(f64::from(f32::EPSILON));
    let spacing = |start: f64, press: f64, now: f64| {
        let snapped = ((start + (now - press) / scale) / DRAG_SNAP_MM).round() * DRAG_SNAP_MM;
        let kept = snapped.clamp(-DRAG_REACH_MM, DRAG_REACH_MM);
        // No "-0" in a field.
        if kept == 0.0 { 0.0 } else { kept }
    };
    (
        spacing(drag.start.0, drag.press.0, at.0),
        spacing(drag.start.1, drag.press.1, at.1),
    )
}

fn pt(x: f64, y: f64) -> Point {
    Point::new(x as f32, y as f32)
}

/// Where everything goes.
pub fn layout(spec: &PreviewSpec) -> PreviewLayout {
    let plan = preview_plan(spec.positions);
    let (w, h) = spec
        .copy_size_mm
        .filter(|(w, h)| w.is_finite() && h.is_finite() && *w > 0.0 && *h > 0.0)
        .unwrap_or((FALLBACK_COPY_MM, FALLBACK_COPY_MM));
    let last = plan.drawn.saturating_sub(1) as f64;
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
    let mut layout = PreviewLayout {
        view_w,
        view_h,
        origin: (ox, oy),
        copy: (w, h),
        spacing: (spec.dx_mm, spec.dy_mm),
        plan,
        margin,
        values: Vec::new(),
    };
    layout.values = drawn_values(&layout, spec.step_label.is_some());
    layout
}

/// The values drawn on the preview. A spacing with a callout is drawn on its
/// callout; a zero spacing, and the Copies value, on the bottom row.
fn drawn_values(layout: &PreviewLayout, counts_up: bool) -> Vec<DrawnValue> {
    let PreviewLayout {
        view_w,
        view_h,
        origin: (ox, oy),
        copy: (w, h),
        spacing: (dx, dy),
        margin,
        ..
    } = *layout;
    let arrayed = layout.plan.drawn >= 2;
    let gap = margin * 0.5;
    let (x1, y1) = (ox + dx, oy + dy);
    let foot = view_h - margin * FOOT_SHARE;
    let value = |field, x: f64, y: f64, anchor| DrawnValue {
        field,
        at: pt(x, y),
        anchor,
    };
    let mut values = vec![value(
        PasteArrayField::Copies,
        margin * EDGE_SHARE,
        foot,
        Anchor::Start,
    )];
    values.push(if arrayed && dx != 0.0 {
        value(
            PasteArrayField::XSpacing,
            ox + dx / 2.0,
            oy.min(y1) - gap,
            Anchor::Middle,
        )
    } else {
        value(
            PasteArrayField::XSpacing,
            view_w / 2.0,
            foot,
            Anchor::Middle,
        )
    });
    values.push(if arrayed && dy != 0.0 {
        value(
            PasteArrayField::YSpacing,
            (ox + w).max(x1 + w) + gap,
            oy + dy / 2.0,
            Anchor::Start,
        )
    } else {
        value(
            PasteArrayField::YSpacing,
            view_w - margin * EDGE_SHARE,
            foot,
            Anchor::End,
        )
    });
    if arrayed && counts_up {
        values.push(value(
            PasteArrayField::Step,
            x1 + w / 2.0,
            oy.max(y1) + h + gap * 0.6,
            Anchor::Middle,
        ));
    }
    values
}

/// The preview as an illustration.
pub fn illustration(spec: PreviewSpec) -> Illustration {
    let layout = layout(&spec);
    let (ox, oy) = layout.origin;
    let (w, h) = layout.copy;
    let plan = layout.plan;
    let last = plan.drawn.saturating_sub(1) as f64;

    let mut parts = grid(spec.grid_mm, ox, oy, layout.view_w, layout.view_h);
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
        parts.extend(callouts(&spec, &layout));
    }
    parts.extend(value_labels(&spec, &layout));
    Illustration::new(layout.view_w as f32, layout.view_h as f32).parts(parts)
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

/// Copy `k` (0 is the original): its outline and its label text. Copy 2,
/// the one a drag moves, is outlined dashed.
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
    let mut style = Style::outline(ink, Weight::Regular).with_fill(Paint::Ink(ink), tint);
    if k == 1
        && let Some(stroke) = style.stroke.as_mut()
    {
        stroke.is_dashed = true;
    }
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
fn callouts(spec: &PreviewSpec, layout: &PreviewLayout) -> Vec<Part> {
    let (ox, oy) = layout.origin;
    let w = layout.copy.0;
    let mut parts = Vec::new();
    let gap = layout.margin * 0.5;
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

/// The drawn values that no callout carries: Copies, the step, and a zero
/// spacing.
fn value_labels(spec: &PreviewSpec, layout: &PreviewLayout) -> Vec<Part> {
    let on_callout = |field| match field {
        PasteArrayField::XSpacing => layout.plan.drawn >= 2 && spec.dx_mm != 0.0,
        PasteArrayField::YSpacing => layout.plan.drawn >= 2 && spec.dy_mm != 0.0,
        PasteArrayField::Copies | PasteArrayField::Step => false,
    };
    layout
        .values
        .iter()
        .filter(|value| !on_callout(value.field))
        .filter_map(|value| {
            let text = match value.field {
                PasteArrayField::Copies => spec.copies_label.clone(),
                PasteArrayField::XSpacing => spec.x_label.clone(),
                PasteArrayField::YSpacing => spec.y_label.clone(),
                PasteArrayField::Step => spec.step_label.clone()?,
            };
            Some(Part::label(
                TextLabel {
                    at: value.at,
                    text,
                    anchor: value.anchor,
                    role: TextRole::Caption,
                },
                Ink::Secondary,
            ))
        })
        .collect()
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
            copies_label: UserText::new(format!("{positions} copies")).into(),
            step_label: Some(UserText::new("+1 per copy".to_owned()).into()),
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

    fn label_texts(ill: &Illustration) -> Vec<String> {
        ill.visible_parts()
            .filter_map(|p| match &p.primitive {
                Primitive::Label(l) => Some(l.text.as_str().to_owned()),
                _ => None,
            })
            .collect()
    }

    /// A 300 x 160 px drawing of `layout`.
    fn fit(layout: &PreviewLayout) -> Fit {
        Fit::new(layout.view_size(), iced::Size::new(300.0, 160.0))
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

    /// Mockup row 69: copies, X, Y and the step are drawn on the drawing; a
    /// zero Y spacing has no callout, so its value sits on the bottom row.
    #[test]
    fn every_value_is_drawn_on_the_drawing() {
        let ill = illustration(spec(4));
        let texts = label_texts(&ill);
        for wanted in ["4 copies", "Y 0 mm", "+1 per copy"] {
            assert!(texts.iter().any(|t| t == wanted), "{wanted:?} in {texts:?}");
        }
        let fields: Vec<_> = layout(&spec(4)).values.iter().map(|v| v.field).collect();
        assert_eq!(
            fields,
            [
                PasteArrayField::Copies,
                PasteArrayField::XSpacing,
                PasteArrayField::YSpacing,
                PasteArrayField::Step,
            ]
        );

        let mut off = spec(4);
        off.step_label = None;
        assert!(
            !label_texts(&illustration(off))
                .iter()
                .any(|t| t == "+1 per copy")
        );
    }

    /// Copy 2, the one a drag moves, is outlined dashed; no other copy is.
    #[test]
    fn copy_two_is_drawn_dashed() {
        let ill = illustration(spec(4));
        let dashed: Vec<bool> = ill
            .visible_parts()
            .filter(|p| matches!(p.primitive, Primitive::RoundedRect { .. }))
            .map(|p| p.style.stroke.is_some_and(|s| s.is_dashed))
            .collect();
        assert_eq!(dashed, [false, true, false, false]);
    }

    /// A press on a drawn value names its field; a press on copy 2 is a grab.
    #[test]
    fn a_press_finds_the_value_or_copy_two_under_it() {
        let layout = layout(&spec(4));
        let fit = fit(&layout);
        for value in &layout.values {
            let (at, _) = layout.value_point(&fit, value.field).unwrap();
            let inside = Point::new(at.x + 1.0, at.y - 2.0);
            let inside = match value.anchor {
                Anchor::End => Point::new(at.x - 4.0, at.y - 2.0),
                _ => inside,
            };
            assert_eq!(layout.value_at(&fit, 11.0, inside), Some(value.field));
        }
        let (ox, oy) = layout.origin;
        let (w, h) = layout.copy;
        let centre_of = |k: f64| {
            fit.point(pt(
                ox + k * layout.spacing.0 + w / 2.0,
                oy + k * layout.spacing.1 + h / 2.0,
            ))
        };
        assert!(layout.is_on_copy_two(&fit, 11.0, centre_of(1.0)));
        assert!(
            !layout.is_on_copy_two(&fit, 11.0, centre_of(0.0)),
            "the original stays"
        );
        assert!(
            !layout.is_on_copy_two(&fit, 11.0, centre_of(2.0)),
            "copy 3 is not grabbed"
        );
        assert_eq!(layout.value_at(&fit, 11.0, centre_of(0.0)), None);
    }

    /// Mockup row 69: dragging copy 2 sets the spacing, snapped to 1.27 mm.
    #[test]
    fn a_drag_sets_the_spacing_on_a_1_27_mm_step() {
        // Pressed at 10 px per mm with the spacing at (10, 1), moved 2.5 mm
        // right and 4.9 mm up: (12.5, -3.9) snaps to (12.7, -3.81).
        let drag = PreviewDrag {
            press: (100.0, 50.0),
            scale: 10.0,
            start: (10.0, 1.0),
        };
        let (dx, dy) = dragged_spacing(drag, (125.0, 1.0));
        assert!((dx - 12.7).abs() < 1e-9, "{dx}");
        assert!((dy + 3.81).abs() < 1e-9, "{dy}");
        let still = PreviewDrag {
            start: (0.0, 0.0),
            ..drag
        };
        let (dx, dy) = dragged_spacing(still, (103.0, 48.0));
        assert_eq!((dx, dy), (0.0, 0.0));
        assert!(dy.is_sign_positive(), "no -0 in the field");
    }

    /// Mockup row 69: a drag keeps the spacing within 254 mm either way.
    #[test]
    fn a_drag_keeps_the_spacing_within_254_mm() {
        let drag = PreviewDrag {
            press: (0.0, 0.0),
            scale: 1.0,
            start: (250.0, -250.0),
        };
        assert_eq!(
            dragged_spacing(drag, (1.0e6, -1.0e6)),
            (DRAG_REACH_MM, -DRAG_REACH_MM)
        );
        assert_eq!(
            dragged_spacing(drag, (-1.0e6, 1.0e6)),
            (-DRAG_REACH_MM, DRAG_REACH_MM)
        );
    }
}
