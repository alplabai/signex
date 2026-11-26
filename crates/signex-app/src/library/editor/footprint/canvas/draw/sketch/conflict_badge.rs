//! The canvas conflict badge (Caner 2026-11-20): no card over the drawing,
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
    let paint = LabelPaint {
        fill: ink.danger,
        border: ink.danger,
        border_px: 1.0,
        text: ink.fill,
        caret: false,
    };
    draw_label(frame, badge_rect(by, text), text, paint);
}

/// Where the badge reading `text` sits: just right of `by`, level with it.
fn badge_rect(by: &Glyph, text: &str) -> Rectangle {
    let width = text.chars().count() as f32 * GLYPH_TEXT * ADVANCE_EM + 2.0 * GLYPH_TEXT * PAD_EM;
    let height = GLYPH_TEXT * HEIGHT_EM;
    let left = by.rect.x + by.rect.width + GLYPH_TEXT * GAP_EM;
    let top = by.rect.y + by.rect.height / 2.0 - height / 2.0;
    Rectangle::new(Point::new(left, top), Size::new(width, height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::editor::footprint::canvas::glyph_layout::GlyphTarget;

    fn glyph(id: ConstraintId, face: GlyphFace, x: f32) -> Glyph {
        Glyph {
            target: GlyphTarget::Constraint(id),
            face,
            anchor: Point::new(x, 10.0),
            rect: Rectangle::new(Point::new(x, 0.0), Size::new(30.0, 20.0)),
        }
    }

    fn value() -> GlyphFace {
        GlyphFace::Value {
            text: "0.70 mm".into(),
            editing: false,
            invalid: false,
            reference: false,
            span: None,
        }
    }

    /// Mockup row 187 (Caner 2026-11-20): no card over the drawing, a red
    /// badge by the conflicting dimension, else by any conflicting glyph.
    #[test]
    fn the_badge_sits_right_of_the_conflicting_value() {
        let (quiet, mark, clash) = (
            ConstraintId::new(),
            ConstraintId::new(),
            ConstraintId::new(),
        );
        let glyphs = vec![
            glyph(quiet, value(), 0.0),
            glyph(mark, GlyphFace::Mark { text: "H" }, 50.0),
            glyph(clash, value(), 100.0),
        ];
        let anchor = |set: &[ConstraintId]| {
            let set: HashSet<ConstraintId> = set.iter().copied().collect();
            badge_anchor(&glyphs, &set).and_then(Glyph::constraint)
        };
        assert_eq!(anchor(&[mark, clash]), Some(clash), "a value comes first");
        assert_eq!(anchor(&[mark]), Some(mark), "else the conflicting mark");
        assert_eq!(anchor(&[]), None, "no conflict, no badge");

        let by = &glyphs[2];
        let rect = badge_rect(by, "1 conflict · see Properties");
        assert!(rect.x > by.rect.x + by.rect.width, "right of the value");
        let middle = by.rect.y + by.rect.height / 2.0;
        assert!(
            rect.y < middle && rect.y + rect.height > middle,
            "level with it"
        );
    }
}
