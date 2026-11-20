//! Values on the drawing (#187): each dimension shows `name · value` in a
//! small label on its dimension line, a value being edited shows the typed
//! text with a caret, and a selected pattern shows its count and spacing
//! the same way. A reference (driven) dimension shows the value it measures
//! and draws its label and line dashed in the reference ink (#188).
//! Pattern glyphs, instance slots and the conflict badge draw from here
//! too, on top of the geometry, all from one layout (`canvas::glyph_layout`).

use iced::widget::canvas::{self, LineDash, Path, Stroke};
use iced::{Color, Point, Rectangle, Size};

use super::arrows::draw_arrow_head;
use super::conflict_badge::{conflict_set, draw_conflict_badge};
use super::pattern_glyphs::{draw_instance, draw_pattern_glyph};
use super::sketch_ink::SketchInk;
use crate::library::editor::footprint::canvas::FootprintCanvasState;
use crate::library::editor::footprint::canvas::glyph_layout::{
    GLYPH_TEXT, Glyph, GlyphFace, sketch_glyphs,
};
use crate::library::editor::footprint::state::FootprintEditorState;

/// Corner radius of a label, in text heights.
const RADIUS_EM: f32 = 0.35;
/// Border width of a label at rest and while edited, in px.
const BORDER_PX: f32 = 1.0;
const BORDER_ACTIVE_PX: f32 = 1.6;
/// Dash of a reference dimension's label border and line, in px (the
/// mockups' `3 2`).
const REFERENCE_DASH: [f32; 2] = [3.0, 2.0];
/// Width of a dimension line, and the length of its arrowheads, in px.
const DIMENSION_LINE_PX: f32 = 1.0;
const ARROW_HEAD_PX: f32 = 5.0;

/// The colours labels draw in on the board ground.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct LabelInk {
    /// Inside of a label, so lines under it do not show through.
    pub fill: Color,
    /// A label's border at rest.
    pub border: Color,
    /// Label text.
    pub text: Color,
    /// An edited value, a selected pattern, an active instance slot.
    pub accent: Color,
    /// A conflicting value, an unreadable edit, the conflict badge.
    pub danger: Color,
    /// Dimension lines.
    pub dimension: Color,
    /// A reference (driven) dimension's label and line.
    pub reference: Color,
}

impl LabelInk {
    pub(super) fn new(danger: Color) -> Self {
        let ink = SketchInk::BOARD;
        Self {
            fill: ink.pill,
            border: ink.pill_border,
            text: ink.label,
            accent: ink.selection,
            danger,
            dimension: ink.dimension,
            reference: ink.reference,
        }
    }
}

/// How one label is painted.
#[derive(Debug, Clone, Copy)]
pub(super) struct LabelPaint {
    pub fill: Color,
    pub border: Color,
    pub border_px: f32,
    pub text: Color,
    /// Draw the typing caret after the text.
    pub caret: bool,
}

/// A rounded label in `rect` with `text` centred in it.
pub(super) fn draw_label(
    frame: &mut canvas::Frame,
    rect: Rectangle,
    text: &str,
    paint: LabelPaint,
) {
    draw_label_with(frame, rect, text, paint, false);
}

/// [`draw_label`], its border dashed when `dashed`.
fn draw_label_with(
    frame: &mut canvas::Frame,
    rect: Rectangle,
    text: &str,
    paint: LabelPaint,
    dashed: bool,
) {
    let shape = Path::rounded_rectangle(
        Point::new(rect.x, rect.y),
        Size::new(rect.width, rect.height),
        (GLYPH_TEXT * RADIUS_EM).into(),
    );
    frame.fill(&shape, paint.fill);
    let border = Stroke::default()
        .with_width(paint.border_px)
        .with_color(paint.border);
    frame.stroke(&shape, if dashed { dash(border) } else { border });
    let centre = Point::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
    frame.fill_text(canvas::Text {
        content: text.to_string(),
        position: centre,
        color: paint.text,
        size: iced::Pixels(GLYPH_TEXT),
        align_x: iced::alignment::Horizontal::Center.into(),
        align_y: iced::alignment::Vertical::Center,
        ..canvas::Text::default()
    });
    if paint.caret {
        let x = rect.x + rect.width - GLYPH_TEXT * 0.45;
        let half = GLYPH_TEXT * 0.5;
        frame.stroke(
            &Path::line(
                Point::new(x, centre.y - half),
                Point::new(x, centre.y + half),
            ),
            Stroke::default()
                .with_width(BORDER_ACTIVE_PX)
                .with_color(paint.border),
        );
    }
}

/// `stroke`, dashed as a reference dimension is.
fn dash(stroke: Stroke<'static>) -> Stroke<'static> {
    Stroke {
        line_dash: LineDash {
            segments: &REFERENCE_DASH,
            offset: 0,
        },
        ..stroke
    }
}

/// How a value label is drawn: its paint, and whether its border is
/// dashed (a reference dimension).
#[derive(Debug, Clone, Copy)]
struct ValueLook {
    paint: LabelPaint,
    dashed: bool,
}

/// The look of a value label.
fn value_look(
    ink: &LabelInk,
    editing: bool,
    invalid: bool,
    conflicting: bool,
    reference: bool,
) -> ValueLook {
    let (border, border_px) = match (editing, invalid || conflicting) {
        (_, true) => (ink.danger, BORDER_ACTIVE_PX),
        (true, false) => (ink.accent, BORDER_ACTIVE_PX),
        (false, false) if reference => (ink.reference, BORDER_ACTIVE_PX),
        (false, false) => (ink.border, BORDER_PX),
    };
    let text = if conflicting && !editing {
        ink.danger
    } else if reference {
        ink.reference
    } else {
        ink.text
    };
    ValueLook {
        paint: LabelPaint {
            fill: ink.fill,
            border,
            border_px,
            text,
            caret: editing,
        },
        dashed: reference,
    }
}

/// The colour and dash of a dimension line.
fn dimension_stroke(ink: &LabelInk, conflicting: bool, reference: bool) -> Stroke<'static> {
    let colour = if conflicting {
        ink.danger
    } else if reference {
        ink.reference
    } else {
        ink.dimension
    };
    let stroke = Stroke::default()
        .with_width(DIMENSION_LINE_PX)
        .with_color(colour);
    if reference { dash(stroke) } else { stroke }
}

/// A dimension line from `a` to `b` with an arrowhead at each end.
fn draw_dimension_line(frame: &mut canvas::Frame, (a, b): (Point, Point), stroke: Stroke<'_>) {
    if a.distance(b) < 1.0 {
        return;
    }
    frame.stroke(&Path::line(a, b), stroke);
    let solid = Stroke {
        line_dash: LineDash::default(),
        ..stroke
    };
    draw_arrow_head(frame, a, b, ARROW_HEAD_PX, solid);
    draw_arrow_head(frame, b, a, ARROW_HEAD_PX, solid);
}

fn draw_value(frame: &mut canvas::Frame, glyph: &Glyph, ink: &LabelInk, conflicting: bool) {
    let GlyphFace::Value {
        text,
        editing,
        invalid,
        reference,
        span,
    } = &glyph.face
    else {
        return;
    };
    if let Some(span) = span {
        draw_dimension_line(frame, *span, dimension_stroke(ink, conflicting, *reference));
    }
    let look = value_look(ink, *editing, *invalid, conflicting, *reference);
    draw_label_with(frame, glyph.rect, text, look.paint, look.dashed);
}

/// Draw the sketch's values, pattern glyphs, instance slots and conflict
/// badge, over everything else the sketch draws.
pub(in crate::library::editor::footprint::canvas::draw) fn draw_sketch_values(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    sketch: &signex_sketch::SketchData,
    state: &FootprintEditorState,
    conflict_colour: Color,
) {
    let glyphs = sketch_glyphs(sketch, state, cstate);
    let ink = LabelInk::new(conflict_colour);
    let conflicts = conflict_set(state);
    for glyph in &glyphs {
        match &glyph.face {
            GlyphFace::Value { .. } => {
                let conflicting = glyph.constraint().is_some_and(|id| conflicts.contains(&id));
                draw_value(frame, glyph, &ink, conflicting);
            }
            GlyphFace::Pattern { .. } => draw_pattern_glyph(frame, glyph, &ink),
            GlyphFace::Instance { .. } => draw_instance(frame, glyph, &ink),
            GlyphFace::Mark { .. } => {}
        }
    }
    draw_conflict_badge(frame, &glyphs, &conflicts, state, &ink);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edited_value_draws_in_the_accent_and_an_unreadable_one_in_danger() {
        let ink = LabelInk::new(Color::from_rgb(1.0, 0.0, 0.0));
        let paint = |editing, invalid, conflicting| {
            value_look(&ink, editing, invalid, conflicting, false).paint
        };
        assert_eq!(paint(true, false, false).border, ink.accent);
        assert!(paint(true, false, false).caret);
        assert_eq!(paint(true, true, false).border, ink.danger);
        assert_eq!(paint(false, false, true).text, ink.danger);
        assert_eq!(paint(false, false, false).border, ink.border);
    }

    #[test]
    fn a_reference_value_draws_dashed_in_the_reference_ink() {
        let ink = LabelInk::new(Color::from_rgb(1.0, 0.0, 0.0));
        assert_eq!(ink.reference, SketchInk::BOARD.reference);
        let look = value_look(&ink, false, false, false, true);
        assert!(look.dashed);
        assert_eq!(look.paint.border, ink.reference);
        assert_eq!(look.paint.text, ink.reference);
        let line = dimension_stroke(&ink, false, true);
        assert_eq!(line.line_dash.segments, REFERENCE_DASH);

        let driving = value_look(&ink, false, false, false, false);
        assert!(!driving.dashed);
        assert_eq!(driving.paint.text, ink.text);
        assert!(
            dimension_stroke(&ink, false, false)
                .line_dash
                .segments
                .is_empty(),
            "a driving dimension's line is solid"
        );
    }

    #[test]
    fn the_board_ground_labels_take_the_mockup_pill() {
        let ink = LabelInk::new(Color::from_rgb(1.0, 0.0, 0.0));
        assert_eq!(ink.fill, SketchInk::BOARD.pill);
        assert_eq!(ink.text, SketchInk::BOARD.label);
        assert_eq!(ink.border, SketchInk::BOARD.pill_border);
        assert_eq!(ink.dimension, SketchInk::BOARD.dimension);
    }
}
