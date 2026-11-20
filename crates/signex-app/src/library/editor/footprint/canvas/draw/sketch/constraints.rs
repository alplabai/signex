//! Constraint-mark overlay: each relation's mark (H, V, ∥, ⊥, =, ◡, ⇔, ⌖
//! …) in a small upright pill by the entities it touches, the coincident
//! mark as a dot (mockup row 187, mockup row 188). A conflicting or over-constrained mark
//! fills in the conflict colour. Dimensions draw their value instead
//! (`dimension_labels`); both read their place from
//! [`crate::library::editor::footprint::canvas::glyph_layout`].

use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point, Size};

use super::sketch_ink::SketchInk;
use crate::library::editor::footprint::canvas::glyph_layout::{
    COINCIDENT_MARK, Glyph, GlyphFace, MARK_TEXT,
};
use crate::library::editor::footprint::state::FootprintEditorState;

/// Corner radius of a mark's pill, in mark text heights (the mockups' 3 px
/// on an 11 px mark).
const PILL_RADIUS_EM: f32 = 0.27;
/// Radius of the coincident dot, in mark text heights.
const DOT_RADIUS_EM: f32 = 0.3;
/// Alpha of a mark the hovered conflict row is not about.
const DIMMED_ALPHA: f32 = 0.3;

/// How one mark is painted.
#[derive(Debug, Clone, Copy, PartialEq)]
struct MarkPaint {
    fill: Color,
    border: Color,
    text: Color,
}

/// The paint of a mark: in conflict it fills with `conflict_colour` and
/// its text takes the pill colour; while a conflict row is hovered, every
/// other mark dims.
fn mark_paint(
    ink: &SketchInk,
    conflict_colour: Color,
    conflicting: bool,
    dimmed: bool,
) -> MarkPaint {
    let paint = if conflicting {
        MarkPaint {
            fill: conflict_colour,
            border: conflict_colour,
            text: ink.pill,
        }
    } else {
        MarkPaint {
            fill: ink.pill,
            border: ink.pill_border,
            text: ink.label,
        }
    };
    if !dimmed {
        return paint;
    }
    let dim = |c: Color| Color {
        a: c.a * DIMMED_ALPHA,
        ..c
    };
    MarkPaint {
        fill: dim(paint.fill),
        border: dim(paint.border),
        text: dim(paint.text),
    }
}

/// Render the constraint marks laid out in `glyphs` under the sketch
/// entities; conflicting and over-constrained ones in `conflict_colour`.
pub(super) fn draw_constraint_icons(
    frame: &mut canvas::Frame,
    glyphs: &[Glyph],
    state: &FootprintEditorState,
    ink: &SketchInk,
    conflict_colour: Color,
) {
    use signex_sketch::id::ConstraintId;

    let over_set: std::collections::HashSet<ConstraintId> = state
        .last_solve
        .as_ref()
        .map(|s| s.over_constraints.iter().copied().collect())
        .unwrap_or_default();

    for glyph in glyphs {
        let (GlyphFace::Mark { text }, Some(id)) = (&glyph.face, glyph.constraint()) else {
            continue;
        };
        // A hovered Conflicts row isolates its constraint: it alone shows
        // in the conflict colour and every other mark dims. With no row
        // hovered the whole over-constrained set shows. A rejected edit's
        // conflict always shows.
        let hover = state.conflicts_row_hovered;
        let rejected = state.conflict.contains_constraint(id);
        let conflicting = rejected || hover.map_or(over_set.contains(&id), |h| h == id);
        let dimmed = !rejected && hover.is_some_and(|h| h != id);
        let paint = mark_paint(ink, conflict_colour, conflicting, dimmed);
        let rect = glyph.rect;
        let centre = Point::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
        if *text == COINCIDENT_MARK {
            let colour = if conflicting { paint.fill } else { paint.text };
            frame.fill(&Path::circle(centre, MARK_TEXT * DOT_RADIUS_EM), colour);
            continue;
        }
        let pill = Path::rounded_rectangle(
            Point::new(rect.x, rect.y),
            Size::new(rect.width, rect.height),
            (MARK_TEXT * PILL_RADIUS_EM).into(),
        );
        frame.fill(&pill, paint.fill);
        frame.stroke(
            &pill,
            Stroke::default().with_width(1.0).with_color(paint.border),
        );
        frame.fill_text(canvas::Text {
            content: (*text).to_string(),
            position: centre,
            color: paint.text,
            size: iced::Pixels(MARK_TEXT),
            font: iced::Font {
                weight: iced::font::Weight::Semibold,
                ..iced::Font::MONOSPACE
            },
            align_x: iced::alignment::Horizontal::Center.into(),
            align_y: iced::alignment::Vertical::Center,
            ..canvas::Text::default()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mark_at_rest_is_a_pill_in_the_label_ink_and_a_conflict_fills_red() {
        let red = Color::from_rgb(1.0, 0.0, 0.0);
        let ink = SketchInk::board();
        let rest = mark_paint(&ink, red, false, false);
        assert_eq!(rest.fill, ink.pill);
        assert_eq!(rest.border, ink.pill_border);
        assert_eq!(rest.text, ink.label);

        let clash = mark_paint(&ink, red, true, false);
        assert_eq!(clash.fill, red, "a conflict fills in the conflict colour");
        assert_eq!(clash.text, ink.pill);

        let dimmed = mark_paint(&ink, red, false, true);
        assert!(dimmed.text.a < rest.text.a, "the other marks dim");
    }
}
