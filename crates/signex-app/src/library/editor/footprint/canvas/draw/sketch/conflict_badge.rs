//! The canvas conflict badge (decision fp-15): no card over the drawing,
//! just a compact red badge next to the conflicting dimension (or the
//! first conflicting glyph) that counts the conflicts and points to
//! Properties, where Remove acts on them.

use std::collections::HashSet;

use iced::widget::canvas;
use iced::{Point, Rectangle, Size};
use signex_sketch::id::ConstraintId;

use super::dimension_labels::{LabelInk, LabelPaint, draw_label};
use crate::library::editor::footprint::canvas::glyph_layout::{GLYPH_TEXT, Glyph, GlyphFace};
use crate::library::editor::footprint::state::FootprintEditorState;

/// Gap between the glyph and the badge, in text heights.
const GAP_EM: f32 = 0.5;
/// Average advance of a badge character, in ems.
const ADVANCE_EM: f32 = 0.6;
/// Badge height and horizontal padding, in text heights.
const HEIGHT_EM: f32 = 1.6;
const PAD_EM: f32 = 0.6;

/// Every constraint in conflict, as the constraint menu reads it.
pub(super) fn conflict_set(state: &FootprintEditorState) -> HashSet<ConstraintId> {
    crate::library::editor::footprint::sketch_mode::value_edit::conflicting_constraints(state)
}

/// The glyph the badge sits by: the first conflicting value, else the
/// first conflicting glyph of any kind.
pub(super) fn badge_anchor<'g>(
    glyphs: &'g [Glyph],
    conflicts: &HashSet<ConstraintId>,
) -> Option<&'g Glyph> {
    let conflicting = |g: &&Glyph| g.constraint().is_some_and(|id| conflicts.contains(&id));
    glyphs
        .iter()
        .filter(conflicting)
        .find(|g| matches!(g.face, GlyphFace::Value { .. }))
        .or_else(|| glyphs.iter().find(conflicting))
}

/// Draw the badge, when anything is in conflict and has a glyph.
pub(super) fn draw_conflict_badge(
    frame: &mut canvas::Frame,
    glyphs: &[Glyph],
    conflicts: &HashSet<ConstraintId>,
    state: &FootprintEditorState,
    ink: &LabelInk,
) {
    if conflicts.is_empty() {
        return;
    }
    let Some(by) = badge_anchor(glyphs, conflicts) else {
        return;
    };
    // Rendered by the app from the conflict count (`refresh_canvas_text`).
    let text = state.canvas_text.conflict_badge.as_str();
    if text.is_empty() {
        return;
    }
    let width = text.chars().count() as f32 * GLYPH_TEXT * ADVANCE_EM + 2.0 * GLYPH_TEXT * PAD_EM;
    let height = GLYPH_TEXT * HEIGHT_EM;
    let left = by.rect.x + by.rect.width + GLYPH_TEXT * GAP_EM;
    let top = by.rect.y + by.rect.height / 2.0 - height / 2.0;
    let rect = Rectangle::new(Point::new(left, top), Size::new(width, height));
    let paint = LabelPaint {
        fill: ink.danger,
        border: ink.danger,
        border_px: 1.0,
        text: ink.fill,
        caret: false,
    };
    draw_label(frame, rect, text, paint);
}
