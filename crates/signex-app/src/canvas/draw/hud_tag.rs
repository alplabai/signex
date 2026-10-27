//! Geometry of the tool-name chip that follows the placement cursor.
//! One home for the numbers so the chip and its text cannot drift.

use iced::{Point, Rectangle, Size};

/// Chip offset from the cursor landing point.
const OFFSET: Point = Point::new(14.0, -16.0);
/// Estimated advance of one glyph at [`FONT_SIZE`] in the HUD font.
const CHAR_ADVANCE: f32 = 7.0;
/// Chip width not accounted for by glyphs (side padding).
const WIDTH_PAD: f32 = 10.0;
const HEIGHT: f32 = 16.0;
/// Chip origin sits this far up-left of the tag origin.
const CHIP_BLEED: f32 = 2.0;
/// Text origin inside the tag.
const TEXT_INSET: Point = Point::new(3.0, 1.0);
pub(super) const FONT_SIZE: f32 = 11.0;

pub(super) struct HudTag {
    pub(super) chip: Rectangle,
    pub(super) text_origin: Point,
}

pub(super) fn layout(cursor: Point, label: &str) -> HudTag {
    let tag = Point::new(cursor.x + OFFSET.x, cursor.y + OFFSET.y);
    let width = label.chars().count() as f32 * CHAR_ADVANCE + WIDTH_PAD;
    HudTag {
        chip: Rectangle::new(
            Point::new(tag.x - CHIP_BLEED, tag.y - CHIP_BLEED),
            Size::new(width, HEIGHT),
        ),
        text_origin: Point::new(tag.x + TEXT_INSET.x, tag.y + TEXT_INSET.y),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pre-#1495 literals, spelled out.
    fn legacy(cursor: Point, label: &str) -> (Rectangle, Point) {
        let tag_x = cursor.x + 14.0;
        let tag_y = cursor.y - 16.0;
        let tag_w = (label.chars().count() as f32) * 7.0 + 10.0;
        (
            Rectangle::new(Point::new(tag_x - 2.0, tag_y - 2.0), Size::new(tag_w, 16.0)),
            Point::new(tag_x + 3.0, tag_y + 1.0),
        )
    }

    #[test]
    fn hud_tag_matches_the_old_literals() {
        for label in ["", "Wire", "Place Power Port", "Netzeichen"] {
            let cursor = Point::new(120.5, 64.25);
            let tag = layout(cursor, label);
            let (chip, text) = legacy(cursor, label);
            assert_eq!(tag.chip, chip, "{label}");
            assert_eq!(tag.text_origin, text, "{label}");
        }
        assert_eq!(FONT_SIZE, 11.0);
    }
}
