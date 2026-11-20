//! Constraint-glyph overlay: each non-dimension constraint's letter at the
//! centroid of the entities it touches, tinted red when the constraint is
//! over-constrained. Dimensions draw their value instead
//! (`dimension_labels`); both read their place from
//! [`crate::library::editor::footprint::canvas::glyph_layout`].

use iced::widget::canvas;
use iced::{Color, Point, Radians, Vector};

use crate::library::editor::footprint::canvas::glyph_layout::{GLYPH_TEXT, Glyph, GlyphFace};
use crate::library::editor::footprint::state::FootprintEditorState;

/// v0.13.2 Phase 6.6 — render the constraint letters laid out in
/// `glyphs` above the sketch entities' fills; over-constrained ones in red.
pub(super) fn draw_constraint_icons(
    frame: &mut canvas::Frame,
    glyphs: &[Glyph],
    state: &FootprintEditorState,
    conflict_colour: Color,
) {
    use signex_sketch::id::ConstraintId;

    let over_set: std::collections::HashSet<ConstraintId> = state
        .last_solve
        .as_ref()
        .map(|s| s.over_constraints.iter().copied().collect())
        .unwrap_or_default();

    for glyph in glyphs {
        let (GlyphFace::Letter { text, rotation }, Some(id)) = (&glyph.face, glyph.constraint())
        else {
            continue;
        };
        let p = glyph.anchor;
        // v0.23 — per-row precision in the Conflicts list. When the
        // user hovers a specific row, only that constraint renders
        // at full red; every other glyph (including other
        // over-constraints) dims so the offender stands out alone.
        // When no row is hovered, fall back to the v0.22 set-wide
        // isolation (the whole over-constraint set lights up).
        let hover = state.conflicts_row_hovered;
        let is_over = over_set.contains(&id);
        let is_conflict = state.conflict.contains_constraint(id);
        let colour = match (hover, is_over) {
            // A rejected edit's conflict always shows in the error colour.
            _ if is_conflict => conflict_colour,
            // Specific row hovered + this is the row → full red.
            (Some(h), _) if h == id => Color::from_rgba(0.85, 0.10, 0.10, 1.00),
            // Specific row hovered + this is NOT the row → dimmed.
            (Some(_), _) => Color::from_rgba(0.40, 0.40, 0.40, 0.30),
            // No row hover + over-constrained → red (set-wide focus).
            (None, true) => Color::from_rgba(0.85, 0.10, 0.10, 1.00),
            // v0.27 — dark grey reads against both the dark Pads-mode
            // canvas and the white Sketch-mode canvas.
            (None, false) => Color::from_rgba(0.30, 0.30, 0.30, 0.95),
        };
        let text = canvas::Text {
            content: (*text).to_string(),
            // A turned glyph is centred on its anchor; an upright one
            // keeps the small nudge right of and above it.
            position: if rotation.is_some() {
                Point::new(0.0, -10.0)
            } else {
                Point::new(glyph.rect.x, glyph.rect.y)
            },
            color: colour,
            size: iced::Pixels(GLYPH_TEXT),
            ..canvas::Text::default()
        };
        // v0.27 — the glyph follows its line through a full turn while
        // the geometry is dragged; letters with no line stay upright.
        if let Some(angle) = rotation {
            frame.with_save(|inner| {
                inner.translate(Vector::new(p.x, p.y));
                inner.rotate(Radians(*angle));
                inner.fill_text(text);
            });
        } else {
            frame.fill_text(text);
        }
    }
}
