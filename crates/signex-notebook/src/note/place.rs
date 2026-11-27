//! Where a rich note's box sits on the sheet: its Size section gives the box,
//! Justify H and V put the box against the note's position, and the note's
//! turn rotates it about that position (counter-clockwise, y down). One rule
//! for the canvas, the hit box, the SVG and the PDF export.

use signex_types::schematic::{HAlign, TextNote, VAlign};

use super::shape::NoteDrawing;

/// A rich note's box on the sheet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedNote {
    /// The box's width and height, in mm.
    pub box_mm: (f64, f64),
    /// The padding: where the drawing's top-left sits inside the box.
    pub padding_mm: f64,
    /// A Fixed box note whose text does not fit: the overflow is not drawn
    /// and the box carries a corner mark.
    pub clipped: bool,
    anchor: [f64; 2],
    /// The box's top-left, before the turn, relative to the anchor.
    offset: [f64; 2],
    cos: f64,
    sin: f64,
}

impl PlacedNote {
    /// The box of `note` whose text draws `drawing` (`None`: no compile yet,
    /// the box of an empty text).
    #[must_use]
    pub fn of(note: &TextNote, drawing: Option<&NoteDrawing>) -> Self {
        let text = drawing.map_or((0.0, 0.0), |d| (d.width_mm, d.height_mm));
        let layout = note.rich;
        let (w, h) = layout.box_size_mm(text);
        let x = match note.justify_h {
            HAlign::Left => 0.0,
            HAlign::Center => -w / 2.0,
            HAlign::Right => -w,
        };
        let y = match note.justify_v {
            VAlign::Top => 0.0,
            VAlign::Center => -h / 2.0,
            VAlign::Bottom => -h,
        };
        let turn = if note.rotation.is_finite() {
            note.rotation.to_radians()
        } else {
            0.0
        };
        Self {
            box_mm: (w, h),
            padding_mm: layout.padding_mm.max(0.0),
            clipped: layout.clips(text),
            anchor: [note.position.x, note.position.y],
            offset: [x, y],
            cos: turn.cos(),
            sin: turn.sin(),
        }
    }

    /// A point of the box (mm from its top-left) on the sheet.
    #[must_use]
    pub fn to_sheet(&self, local: [f64; 2]) -> [f64; 2] {
        let x = local[0] + self.offset[0];
        let y = local[1] + self.offset[1];
        [
            self.anchor[0] + x * self.cos + y * self.sin,
            self.anchor[1] - x * self.sin + y * self.cos,
        ]
    }

    /// A point of the drawing (mm from the text's top-left) on the sheet.
    #[must_use]
    pub fn text_to_sheet(&self, point: [f64; 2]) -> [f64; 2] {
        self.to_sheet([point[0] + self.padding_mm, point[1] + self.padding_mm])
    }

    /// A sheet point in the drawing's own mm: the inverse of
    /// [`Self::text_to_sheet`], for a click on a link.
    #[must_use]
    pub fn sheet_to_text(&self, sheet: [f64; 2]) -> [f64; 2] {
        let dx = sheet[0] - self.anchor[0];
        let dy = sheet[1] - self.anchor[1];
        let x = dx * self.cos - dy * self.sin;
        let y = dx * self.sin + dy * self.cos;
        [
            x - self.offset[0] - self.padding_mm,
            y - self.offset[1] - self.padding_mm,
        ]
    }

    /// The box's corners on the sheet, clockwise from its top-left.
    #[must_use]
    pub fn corners(&self) -> [[f64; 2]; 4] {
        let (w, h) = self.box_mm;
        [
            self.to_sheet([0.0, 0.0]),
            self.to_sheet([w, 0.0]),
            self.to_sheet([w, h]),
            self.to_sheet([0.0, h]),
        ]
    }

    /// The box's extent on the sheet, `[min_x, min_y, max_x, max_y]`.
    #[must_use]
    pub fn bounds(&self) -> [f64; 4] {
        let corners = self.corners();
        corners.iter().fold(
            [
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ],
            |b, p| {
                [
                    b[0].min(p[0]),
                    b[1].min(p[1]),
                    b[2].max(p[0]),
                    b[3].max(p[1]),
                ]
            },
        )
    }

    /// Whether a shape whose drawing-space bounds are `bounds` is drawn: a
    /// clipped note leaves out every shape that reaches past its box.
    #[must_use]
    pub fn keeps(&self, bounds: [f64; 4]) -> bool {
        if !self.clipped {
            return true;
        }
        let (w, h) = self.box_mm;
        let pad = self.padding_mm;
        bounds[2] + pad <= w + 1e-6 && bounds[3] + pad <= h + 1e-6
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{NoteSize, Point, RichNoteLayout, TextNoteFormat};

    fn note(h: HAlign, v: VAlign, rotation: f64) -> TextNote {
        TextNote {
            uuid: uuid::Uuid::nil(),
            text: String::new(),
            position: Point::new(10.0, 20.0),
            rotation,
            font_size: 1.27,
            font_family: None,
            justify_h: h,
            justify_v: v,
            locked: false,
            format: TextNoteFormat::Typst,
            rich: RichNoteLayout {
                size: NoteSize::FixedBox,
                width_mm: 8.0,
                height_mm: 4.0,
                padding_mm: 1.0,
                ..RichNoteLayout::default()
            },
        }
    }

    #[test]
    fn justify_puts_the_box_against_the_position() {
        let top_left = PlacedNote::of(&note(HAlign::Left, VAlign::Top, 0.0), None);
        assert_eq!(top_left.bounds(), [10.0, 20.0, 18.0, 24.0]);
        assert_eq!(top_left.text_to_sheet([0.0, 0.0]), [11.0, 21.0]);
        let centred = PlacedNote::of(&note(HAlign::Center, VAlign::Bottom, 0.0), None);
        assert_eq!(centred.bounds(), [6.0, 16.0, 14.0, 20.0]);
    }

    #[test]
    fn a_turn_rotates_counter_clockwise_about_the_position() {
        let turned = PlacedNote::of(&note(HAlign::Left, VAlign::Top, 90.0), None);
        let [x, y] = turned.to_sheet([8.0, 0.0]);
        assert!(
            (x - 10.0).abs() < 1e-9 && (y - 12.0).abs() < 1e-9,
            "{x},{y}"
        );
        let back = turned.sheet_to_text(turned.text_to_sheet([2.5, 1.5]));
        assert!((back[0] - 2.5).abs() < 1e-9 && (back[1] - 1.5).abs() < 1e-9);
    }

    #[test]
    fn a_fixed_box_keeps_only_what_fits() {
        let text = NoteDrawing {
            width_mm: 6.0,
            height_mm: 9.0,
            ..NoteDrawing::default()
        };
        let placed = PlacedNote::of(&note(HAlign::Left, VAlign::Top, 0.0), Some(&text));
        assert!(placed.clipped);
        assert!(placed.keeps([0.0, 0.0, 5.0, 2.0]));
        assert!(!placed.keeps([0.0, 2.5, 5.0, 3.5]));
    }
}
