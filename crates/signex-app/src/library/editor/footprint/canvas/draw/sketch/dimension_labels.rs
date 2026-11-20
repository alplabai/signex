//! Values on the drawing (#187): each dimension shows `name · value` in a
//! small label where its letter used to be, a value being edited shows the
//! typed text with a caret, and a selected pattern shows its count and
//! spacing the same way. Pattern glyphs, instance slots and the conflict
//! badge draw from here too, on top of the geometry, all from one layout
//! (`canvas::glyph_layout`).

use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point, Rectangle, Size};

use super::super::cursor_ink::CursorInk;
use super::super::graphite::look_from_bg;
use super::conflict_badge::{conflict_set, draw_conflict_badge};
use super::pattern_glyphs::{draw_instance, draw_pattern_glyph};
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

/// The colours labels draw in, read off the canvas ground so they hold on
/// the board ground and on the white sketch canvas alike.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct LabelInk {
    /// Inside of a label: the ground, so lines under it do not show through.
    pub fill: Color,
    /// A label's border at rest.
    pub border: Color,
    /// Label text.
    pub text: Color,
    /// An edited value, a selected pattern, an active instance slot.
    pub accent: Color,
    /// A conflicting value, an unreadable edit, the conflict badge.
    pub danger: Color,
}

impl LabelInk {
    pub(super) fn new(bg: Color, danger: Color) -> Self {
        let ink = CursorInk::for_sketch_on(bg);
        let ground = if look_from_bg(bg).is_some() {
            bg
        } else {
            Color::WHITE
        };
        let selection = crate::styles::board_canvas_colors().selection;
        Self {
            fill: ground,
            border: Color {
                a: 0.45,
                ..ink.mark
            },
            text: ink.mark,
            accent: crate::render_config::to_iced(&selection),
            danger,
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
    let shape = Path::rounded_rectangle(
        Point::new(rect.x, rect.y),
        Size::new(rect.width, rect.height),
        (GLYPH_TEXT * RADIUS_EM).into(),
    );
    frame.fill(&shape, paint.fill);
    frame.stroke(
        &shape,
        Stroke::default()
            .with_width(paint.border_px)
            .with_color(paint.border),
    );
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

/// The paint of a value label.
fn value_paint(ink: &LabelInk, editing: bool, invalid: bool, conflicting: bool) -> LabelPaint {
    let (border, border_px) = match (editing, invalid || conflicting) {
        (_, true) => (ink.danger, BORDER_ACTIVE_PX),
        (true, false) => (ink.accent, BORDER_ACTIVE_PX),
        (false, false) => (ink.border, BORDER_PX),
    };
    LabelPaint {
        fill: ink.fill,
        border,
        border_px,
        text: if conflicting && !editing {
            ink.danger
        } else {
            ink.text
        },
        caret: editing,
    }
}

fn draw_value(frame: &mut canvas::Frame, glyph: &Glyph, ink: &LabelInk, conflicting: bool) {
    let GlyphFace::Value {
        text,
        editing,
        invalid,
    } = &glyph.face
    else {
        return;
    };
    let paint = value_paint(ink, *editing, *invalid, conflicting);
    draw_label(frame, glyph.rect, text, paint);
}

/// Draw the sketch's values, pattern glyphs, instance slots and conflict
/// badge, over everything else the sketch draws.
pub(in crate::library::editor::footprint::canvas::draw) fn draw_sketch_values(
    frame: &mut canvas::Frame,
    cstate: &FootprintCanvasState,
    sketch: &signex_sketch::SketchData,
    state: &FootprintEditorState,
    bg: Color,
    conflict_colour: Color,
) {
    let glyphs = sketch_glyphs(sketch, state, cstate);
    let ink = LabelInk::new(bg, conflict_colour);
    let conflicts = conflict_set(state);
    for glyph in &glyphs {
        match &glyph.face {
            GlyphFace::Value { .. } => {
                let conflicting = glyph.constraint().is_some_and(|id| conflicts.contains(&id));
                draw_value(frame, glyph, &ink, conflicting);
            }
            GlyphFace::Pattern { .. } => draw_pattern_glyph(frame, glyph, &ink),
            GlyphFace::Instance { .. } => draw_instance(frame, glyph, &ink),
            GlyphFace::Letter { .. } => {}
        }
    }
    draw_conflict_badge(frame, &glyphs, &conflicts, state, &ink);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edited_value_draws_in_the_accent_and_an_unreadable_one_in_danger() {
        let ink = LabelInk::new(Color::BLACK, Color::from_rgb(1.0, 0.0, 0.0));
        assert_eq!(value_paint(&ink, true, false, false).border, ink.accent);
        assert!(value_paint(&ink, true, false, false).caret);
        assert_eq!(value_paint(&ink, true, true, false).border, ink.danger);
        assert_eq!(value_paint(&ink, false, false, true).text, ink.danger);
        assert_eq!(value_paint(&ink, false, false, false).border, ink.border);
    }
}
