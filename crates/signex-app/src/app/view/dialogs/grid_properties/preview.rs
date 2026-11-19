//! The live preview of the Grid Properties dialog: the fine and coarse grid
//! drawn as chosen (Lines / Dots / Hidden), the origin, a pad for scale,
//! and the step and coarse spacing as dimension labels the user can edit
//! on the drawing. Pure: built from the dialog draft and the preview size.
//!
//! The scale is picked so one coarse cell is about a fifth of the width,
//! within [`MIN_PX_PER_MM`]..[`MAX_PX_PER_MM`]; a fine grid whose pitch
//! falls under [`MIN_PITCH_EM`] is not drawn, and a note says so.

use iced::{Point, Rectangle, Size};
use signex_i18n::{I18n, ids, t};
use signex_types::coord::Unit;
use signex_types::measure::length_field_text;
use signex_widgets::LengthFormat;
use signex_widgets::dimension_label::{DimensionLabel, DimensionLabelMessage, LabelAnchor};
use signex_widgets::property_illustration::{
    Anchor, Callout, Illustration, Ink, Paint, Part, Primitive, Style, TextLabel, TextRole, Tint,
    Weight,
};

use crate::app::GridPropertiesState;
use crate::library::editor::footprint::state::GridDisplay;

/// The multipliers the dialog offers, smallest first.
pub(crate) const MULTIPLIERS: [u32; 4] = [1, 2, 5, 10];

/// Preview height in body ems: about 200 px folded, taller expanded.
pub(crate) const HEIGHT_EM: f32 = 16.5;
pub(crate) const EXPANDED_HEIGHT_EM: f32 = 28.0;

/// Share of the width one coarse cell aims for.
const COARSE_SHARE: f32 = 0.2;
/// Bounds of the drawing scale, in px per mm.
const MIN_PX_PER_MM: f32 = 4.0;
const MAX_PX_PER_MM: f32 = 120.0;
/// Closest pitch a grid is drawn at, in ems.
const MIN_PITCH_EM: f32 = 0.75;
/// Origin inset from the bottom-left corner, in ems.
const ORIGIN_EM: f32 = 2.0;
/// Arm of the origin cross, in ems.
const ORIGIN_ARM_EM: f32 = 0.5;
/// Rows of the step and coarse dimensions from the top, in ems.
const STEP_ROW_EM: f32 = 1.6;
const COARSE_ROW_EM: f32 = 3.4;
/// Gap between a dimension and its label, and the room a label needs.
const LABEL_GAP_EM: f32 = 0.6;
const LABEL_ROOM_EM: f32 = 9.0;
/// Dot radii of the fine and coarse grid, in ems.
const FINE_DOT_EM: f32 = 0.06;
const COARSE_DOT_EM: f32 = 0.1;
/// The scale pad, in mm, and where it sits as a share of the size.
const PAD_MM: (f32, f32) = (1.55, 0.6);
const PAD_AT: (f32, f32) = (0.68, 0.45);
/// Decimals the labels show, in mm.
const LABEL_DECIMALS: usize = 3;

/// The preview's size in px and the text size its metrics derive from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PreviewSize {
    pub width: f32,
    pub height: f32,
    pub text_size: f32,
}

impl PreviewSize {
    pub(crate) fn new(width: f32, expanded: bool, text_size: f32) -> Self {
        let em = if expanded {
            EXPANDED_HEIGHT_EM
        } else {
            HEIGHT_EM
        };
        Self {
            width,
            height: em * text_size,
            text_size,
        }
    }

    fn em(&self, ems: f32) -> f32 {
        ems * self.text_size
    }
}

/// How the draft draws at a preview size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Scale {
    pub px_per_mm: f32,
    /// Fine and coarse pitch, in px.
    pub fine: f32,
    pub coarse: f32,
    pub origin: Point,
    pub fine_drawn: bool,
    /// The fine grid is shown but too dense to draw.
    pub fine_too_dense: bool,
    pub coarse_drawn: bool,
}

/// Which value a preview label edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreviewLabel {
    Step,
    Coarse,
}

/// The draft's step, when it is a number above zero.
pub(crate) fn draft_step(state: &GridPropertiesState) -> Option<f64> {
    crate::numeric_field::validated_grid_step(&state.step_mm).ok()
}

pub(crate) fn scale(state: &GridPropertiesState, step_mm: f64, size: PreviewSize) -> Scale {
    let multiplier = state.multiplier.max(1) as f32;
    let coarse_mm = step_mm as f32 * multiplier;
    let px_per_mm = (size.width * COARSE_SHARE / coarse_mm).clamp(MIN_PX_PER_MM, MAX_PX_PER_MM);
    let fine = step_mm as f32 * px_per_mm;
    let coarse = fine * multiplier;
    let min_pitch = size.em(MIN_PITCH_EM);
    let fine_shown = state.fine_display != GridDisplay::Hidden;
    Scale {
        px_per_mm,
        fine,
        coarse,
        origin: Point::new(size.em(ORIGIN_EM), size.height - size.em(ORIGIN_EM)),
        fine_drawn: fine_shown && fine >= min_pitch,
        fine_too_dense: fine_shown && fine < min_pitch,
        coarse_drawn: state.coarse_display != GridDisplay::Hidden && coarse >= min_pitch,
    }
}

/// The multiplier closest to `ratio` (coarse over step); a tie keeps the
/// smaller one.
pub(crate) fn nearest_multiplier(ratio: f64) -> u32 {
    let mut best = MULTIPLIERS[0];
    for m in MULTIPLIERS {
        if (f64::from(m) - ratio).abs() < (f64::from(best) - ratio).abs() {
            best = m;
        }
    }
    best
}

/// Write a value the user set on a preview label into the draft: the step
/// text, or the multiplier closest to the coarse spacing typed. The step
/// is validated on OK, as one typed in the Step field.
pub(crate) fn apply_label(state: &mut GridPropertiesState, label: PreviewLabel, value_mm: f64) {
    match label {
        PreviewLabel::Step => state.step_mm = length_field_text(value_mm, Unit::Mm),
        PreviewLabel::Coarse => {
            if let Some(step) = draft_step(state) {
                state.multiplier = nearest_multiplier(value_mm / step);
            }
        }
    }
}

/// Route a label event: an Enter commits the typed value into the draft.
/// Returns whether a label field opened, so the caller focuses it.
pub(crate) fn label_edited(
    state: &mut GridPropertiesState,
    event: DimensionLabelMessage,
    i18n: &I18n,
) -> bool {
    // Positions do not matter to the update; the values and order do.
    let size = PreviewSize::new(0.0, state.preview_expanded, 1.0);
    let (kinds, labels): (Vec<_>, Vec<_>) = labels(state, size, i18n).into_iter().unzip();
    let outcome = state.preview_labels.update(event, &labels);
    if let Some(done) = outcome.committed
        && let Some(&kind) = kinds.get(done.index)
    {
        apply_label(state, kind, done.value);
    }
    outcome.focus_input
}

/// The labels the preview shows, in order, placed for `size`. None while
/// the step is not a number above zero; no coarse label while the coarse
/// grid is hidden.
pub(crate) fn labels<'a>(
    state: &GridPropertiesState,
    size: PreviewSize,
    i18n: &'a I18n,
) -> Vec<(PreviewLabel, DimensionLabel<'a>)> {
    let Some(step) = draft_step(state) else {
        return Vec::new();
    };
    let s = scale(state, step, size);
    let format = LengthFormat::new(i18n, Unit::Mm);
    let room = size.width - size.em(LABEL_ROOM_EM);
    let gap = size.em(LABEL_GAP_EM);
    let (step_from, step_to) = step_span(&s, size);
    let step_x = if s.fine_drawn {
        step_to + gap
    } else {
        step_from
    };
    let mut out = vec![(
        PreviewLabel::Step,
        DimensionLabel::new(
            step,
            format,
            Point::new(step_x.min(room), size.em(STEP_ROW_EM)),
        )
        .caption(t!(i18n, ids::DIALOG_GRID_PREVIEW_STEP))
        .decimals(LABEL_DECIMALS)
        .anchor(LabelAnchor::Start),
    )];
    if state.coarse_display != GridDisplay::Hidden {
        let multiplier = state.multiplier.max(1);
        let (_, coarse_to) = coarse_span(&s, size);
        out.push((
            PreviewLabel::Coarse,
            DimensionLabel::new(
                step * f64::from(multiplier),
                format,
                Point::new((coarse_to + gap).min(room), size.em(COARSE_ROW_EM)),
            )
            .caption(t!(
                i18n,
                ids::DIALOG_GRID_PREVIEW_COARSE,
                "multiplier" => multiplier
            ))
            .decimals(LABEL_DECIMALS)
            .anchor(LabelAnchor::Start),
        ));
    }
    out
}

/// Where the step dimension runs: one fine pitch, one coarse cell in from
/// the origin when there is room for its label.
fn step_span(s: &Scale, size: PreviewSize) -> (f32, f32) {
    let room = size.width - size.em(LABEL_ROOM_EM);
    let from = if s.origin.x + s.coarse + s.fine <= room {
        s.origin.x + s.coarse
    } else {
        s.origin.x
    };
    (from, from + s.fine)
}

/// Where the coarse dimension runs: one coarse cell, two cells in from the
/// origin when there is room for its label.
fn coarse_span(s: &Scale, size: PreviewSize) -> (f32, f32) {
    let room = size.width - size.em(LABEL_ROOM_EM);
    let from = if s.origin.x + 3.0 * s.coarse <= room {
        s.origin.x + 2.0 * s.coarse
    } else {
        s.origin.x
    };
    (from, from + s.coarse)
}

/// The drawing for the draft at `size`.
pub(crate) fn illustration(
    state: &GridPropertiesState,
    size: PreviewSize,
    i18n: &I18n,
) -> Illustration {
    let drawing =
        Illustration::new(size.width, size.height).height_em(size.height / size.text_size);
    let Some(step) = draft_step(state) else {
        return drawing.part(note(size, t!(i18n, ids::DIALOG_GRID_NO_STEP).into()));
    };
    let s = scale(state, step, size);
    let mut parts = Vec::new();
    if s.fine_drawn {
        grid(&mut parts, &s, s.fine, state.fine_display, size, false);
    }
    if s.coarse_drawn {
        grid(&mut parts, &s, s.coarse, state.coarse_display, size, true);
    }
    origin_cross(&mut parts, &s, size);
    parts.push(pad(&s, size));
    if s.fine_drawn {
        let (from, to) = step_span(&s, size);
        let y = size.em(STEP_ROW_EM);
        parts.push(Part::callout(Callout::new(
            Point::new(from, y),
            Point::new(to, y),
        )));
    }
    if s.coarse_drawn {
        let (from, to) = coarse_span(&s, size);
        let y = size.em(COARSE_ROW_EM);
        parts.push(Part::callout(Callout::new(
            Point::new(from, y),
            Point::new(to, y),
        )));
    }
    if s.fine_too_dense {
        parts.push(note(size, t!(i18n, ids::DIALOG_GRID_TOO_DENSE).into()));
    }
    drawing.parts(parts)
}

/// Grid lines or dots at `pitch` px from the origin across the drawing.
fn grid(
    parts: &mut Vec<Part>,
    s: &Scale,
    pitch: f32,
    display: GridDisplay,
    size: PreviewSize,
    coarse: bool,
) {
    let xs = steps(s.origin.x, pitch, size.width);
    let ys = steps(s.origin.y, pitch, size.height);
    match display {
        GridDisplay::Hidden => {}
        GridDisplay::Lines => {
            let style = if coarse {
                Style::outline(Ink::Faint, Weight::Thin)
            } else {
                Style::outline(Ink::Rule, Weight::Hairline)
            };
            for &x in &xs {
                let line = Primitive::Line {
                    from: Point::new(x, 0.0),
                    to: Point::new(x, size.height),
                };
                parts.push(Part::new(line, style));
            }
            for &y in &ys {
                let line = Primitive::Line {
                    from: Point::new(0.0, y),
                    to: Point::new(size.width, y),
                };
                parts.push(Part::new(line, style));
            }
        }
        GridDisplay::Dots => {
            let (ink, radius) = if coarse {
                (Ink::Tertiary, size.em(COARSE_DOT_EM))
            } else {
                (Ink::Faint, size.em(FINE_DOT_EM))
            };
            let style = Style::filled(Paint::Ink(ink), Tint::Solid);
            for &x in &xs {
                for &y in &ys {
                    let dot = Primitive::Circle {
                        center: Point::new(x, y),
                        radius,
                    };
                    parts.push(Part::new(dot, style));
                }
            }
        }
    }
}

/// Positions `origin + k * pitch` that fall in `0..=extent`.
fn steps(origin: f32, pitch: f32, extent: f32) -> Vec<f32> {
    if pitch <= 0.0 {
        return Vec::new();
    }
    let first = (-origin / pitch).ceil() as i64;
    let last = ((extent - origin) / pitch).floor() as i64;
    (first..=last).map(|k| origin + k as f32 * pitch).collect()
}

fn origin_cross(parts: &mut Vec<Part>, s: &Scale, size: PreviewSize) {
    let arm = size.em(ORIGIN_ARM_EM);
    let o = s.origin;
    let style = Style::outline(Ink::Secondary, Weight::Regular);
    for (from, to) in [
        (Point::new(o.x - arm, o.y), Point::new(o.x + arm, o.y)),
        (Point::new(o.x, o.y - arm), Point::new(o.x, o.y + arm)),
    ] {
        parts.push(Part::new(Primitive::Line { from, to }, style));
    }
}

/// A pad at true scale, centred on the fine node nearest its spot.
fn pad(s: &Scale, size: PreviewSize) -> Part {
    let snap = |at: f32, origin: f32| {
        if s.fine > 0.0 {
            origin + ((at - origin) / s.fine).round() * s.fine
        } else {
            at
        }
    };
    let cx = snap(PAD_AT.0 * size.width, s.origin.x);
    let cy = snap(PAD_AT.1 * size.height, s.origin.y);
    let (w, h) = (PAD_MM.0 * s.px_per_mm, PAD_MM.1 * s.px_per_mm);
    let rect = Rectangle::new(Point::new(cx - w / 2.0, cy - h / 2.0), Size::new(w, h));
    Part::new(
        Primitive::RoundedRect {
            rect,
            radius: h * 0.25,
        },
        Style::outline(Ink::Accent, Weight::Regular).with_fill(Paint::Ink(Ink::Accent), Tint::Body),
    )
}

/// A note in the bottom-right corner.
fn note(size: PreviewSize, text: signex_widgets::property_illustration::Text) -> Part {
    let gap = size.em(LABEL_GAP_EM);
    Part::label(
        TextLabel {
            at: Point::new(size.width - gap, size.height - gap),
            text,
            anchor: Anchor::End,
            role: TextRole::Note,
        },
        Ink::Tertiary,
    )
}
