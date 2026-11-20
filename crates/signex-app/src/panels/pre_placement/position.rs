//! Where the next object goes (#2532): the Position X / Y fields of the
//! pre-placement form, a wire's Start and End, and the anchor the label
//! justification grid names. A typed coordinate wins over the cursor until
//! the object is placed; an untyped one follows the cursor.

use signex_i18n::{MessageId, ids};
use signex_types::schematic::{HAlign, Point, VAlign};

/// One editable coordinate of the form. The wire tool's Start is `X` / `Y`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PositionField {
    X,
    Y,
    EndX,
    EndY,
}

impl PositionField {
    fn index(self) -> usize {
        match self {
            Self::X => 0,
            Self::Y => 1,
            Self::EndX => 2,
            Self::EndY => 3,
        }
    }

    /// A Y coordinate: shown and typed from the sheet's page origin (#2034).
    pub fn is_y(self) -> bool {
        matches!(self, Self::Y | Self::EndY)
    }
}

/// What the Location rows hold beyond the cursor: the typed coordinates
/// (stored sheet values, Y already through the page origin), the text of
/// each field as typed, and the point a wire being drawn starts from.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlacementPosition {
    values: [Option<f64>; 4],
    drafts: [Option<String>; 4],
    /// Fields whose text does not read as a length: OK is refused while
    /// one is, so nothing is placed at a coordinate the field does not show.
    invalid: [bool; 4],
    /// The anchor of the wire being drawn, when one is; the Start fields
    /// show it until a Start coordinate is typed.
    pub wire_anchor: Option<Point>,
}

impl PlacementPosition {
    /// The typed coordinate of `field`, a stored sheet value.
    pub fn value(&self, field: PositionField) -> Option<f64> {
        self.values[field.index()]
    }

    /// The text of `field` while it is being typed.
    pub fn draft(&self, field: PositionField) -> Option<&str> {
        self.drafts[field.index()].as_deref()
    }

    /// Whether any coordinate is typed: OK then places at the typed point,
    /// or is refused while a field does not read.
    pub fn is_typed(&self) -> bool {
        self.values.iter().any(Option::is_some) || self.has_invalid()
    }

    /// Whether the text of `field` does not read as a length.
    pub fn is_invalid(&self, field: PositionField) -> bool {
        self.invalid[field.index()]
    }

    /// Whether any field's text does not read as a length.
    pub fn has_invalid(&self) -> bool {
        self.invalid.iter().any(|bad| *bad)
    }

    /// `text` typed into `field`, `stored` its sheet value when it reads as
    /// a number. An emptied field follows the cursor again; text that does
    /// not read keeps the last typed value but marks the field invalid.
    pub fn typed(&mut self, field: PositionField, text: String, stored: Option<f64>) {
        let i = field.index();
        let empty = text.trim().is_empty();
        if empty {
            self.values[i] = None;
        } else if let Some(value) = stored {
            self.values[i] = Some(value);
        }
        self.invalid[i] = !empty && stored.is_none();
        self.drafts[i] = Some(text);
    }

    /// Forget every typed coordinate: the next object starts from the cursor.
    pub fn clear_typed(&mut self) {
        self.values = [None; 4];
        self.drafts = Default::default();
        self.invalid = [false; 4];
    }

    /// The point the object (or a wire's Start) is placed at: the typed
    /// coordinates over `fallback`.
    pub fn start(&self, fallback: Point) -> Point {
        Point::new(
            self.value(PositionField::X).unwrap_or(fallback.x),
            self.value(PositionField::Y).unwrap_or(fallback.y),
        )
    }

    /// A wire's End: the typed coordinates over `fallback`.
    pub fn end(&self, fallback: Point) -> Point {
        Point::new(
            self.value(PositionField::EndX).unwrap_or(fallback.x),
            self.value(PositionField::EndY).unwrap_or(fallback.y),
        )
    }

    /// A wire's Start before any typing: the anchor of the wire being drawn,
    /// else `cursor`.
    pub fn live_start(&self, cursor: Point) -> Point {
        self.start(self.wire_anchor.unwrap_or(cursor))
    }
}

/// Messages of the Location rows and the justification grid.
#[derive(Debug, Clone, PartialEq)]
pub enum PrePlacementPositionMsg {
    /// A keystroke in a coordinate field: the text as it now reads.
    FieldTyped(PositionField, String),
    /// A cell of the justification grid was picked.
    AnchorPicked(HAlign, VAlign),
}

/// The length of the wire from `start` to `end`, in mm.
pub fn wire_length(start: Point, end: Point) -> f64 {
    (end.x - start.x).hypot(end.y - start.y)
}

/// The rows of the justification grid, top to bottom, and the columns.
pub const ANCHOR_ROWS: [VAlign; 3] = [VAlign::Top, VAlign::Center, VAlign::Bottom];
pub const ANCHOR_COLUMNS: [HAlign; 3] = [HAlign::Left, HAlign::Center, HAlign::Right];

/// The catalog name of the anchor (`h`, `v`), as the grid caption says it.
pub fn anchor_name(h: HAlign, v: VAlign) -> MessageId {
    match (v, h) {
        (VAlign::Top, HAlign::Left) => ids::PROPERTIES_ANCHOR_TOP_LEFT,
        (VAlign::Top, HAlign::Center) => ids::PROPERTIES_ANCHOR_TOP,
        (VAlign::Top, HAlign::Right) => ids::PROPERTIES_ANCHOR_TOP_RIGHT,
        (VAlign::Center, HAlign::Left) => ids::PROPERTIES_ANCHOR_LEFT,
        (VAlign::Center, HAlign::Center) => ids::PROPERTIES_ANCHOR_CENTER,
        (VAlign::Center, HAlign::Right) => ids::PROPERTIES_ANCHOR_RIGHT,
        (VAlign::Bottom, HAlign::Left) => ids::PROPERTIES_ANCHOR_BOTTOM_LEFT,
        (VAlign::Bottom, HAlign::Center) => ids::PROPERTIES_ANCHOR_BOTTOM,
        (VAlign::Bottom, HAlign::Right) => ids::PROPERTIES_ANCHOR_BOTTOM_RIGHT,
    }
}

pub use crate::panels::rotation_steps::{ROTATION_STEPS, exact_rotation_step as rotation_step};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_typed_coordinate_wins_over_the_cursor_until_cleared() {
        let mut pos = PlacementPosition::default();
        let cursor = Point::new(1.0, 2.0);
        assert!(!pos.is_typed());
        assert_eq!(pos.start(cursor), cursor);
        pos.typed(PositionField::X, "10.5".into(), Some(10.5));
        assert!(pos.is_typed());
        assert_eq!(pos.start(cursor), Point::new(10.5, 2.0));
        assert_eq!(pos.draft(PositionField::X), Some("10.5"));
        pos.clear_typed();
        assert!(!pos.is_typed());
        assert_eq!(pos.draft(PositionField::X), None);
        assert_eq!(pos.start(cursor), cursor);
    }

    #[test]
    fn text_that_does_not_read_keeps_the_last_value_and_empty_follows_the_cursor() {
        let mut pos = PlacementPosition::default();
        pos.typed(PositionField::EndY, "4".into(), Some(4.0));
        pos.typed(PositionField::EndY, "4x".into(), None);
        assert_eq!(pos.value(PositionField::EndY), Some(4.0));
        assert_eq!(pos.draft(PositionField::EndY), Some("4x"));
        pos.typed(PositionField::EndY, " ".into(), None);
        assert_eq!(pos.value(PositionField::EndY), None);
        assert!(!pos.is_typed());
    }

    /// "12a" marks its field invalid (OK is refused) until it reads again,
    /// even with no earlier value to keep.
    #[test]
    fn text_that_does_not_read_marks_the_field_invalid() {
        let mut pos = PlacementPosition::default();
        pos.typed(PositionField::X, "12a".into(), None);
        assert!(pos.is_invalid(PositionField::X) && pos.has_invalid());
        assert!(pos.is_typed(), "OK must answer, not place at the cursor");
        assert_eq!(pos.value(PositionField::X), None);
        pos.typed(PositionField::X, "12".into(), Some(12.0));
        assert!(!pos.has_invalid());
        pos.typed(PositionField::Y, "x".into(), None);
        pos.clear_typed();
        assert!(!pos.has_invalid());
    }

    #[test]
    fn a_wire_start_is_its_anchor_until_typed() {
        let mut pos = PlacementPosition {
            wire_anchor: Some(Point::new(5.0, 6.0)),
            ..Default::default()
        };
        let cursor = Point::new(9.0, 9.0);
        assert_eq!(pos.live_start(cursor), Point::new(5.0, 6.0));
        pos.typed(PositionField::Y, "7".into(), Some(7.0));
        assert_eq!(pos.live_start(cursor), Point::new(5.0, 7.0));
        pos.wire_anchor = None;
        assert_eq!(pos.live_start(cursor), Point::new(9.0, 7.0));
        assert_eq!(pos.end(cursor), cursor);
    }

    #[test]
    fn length_is_derived_from_start_and_end() {
        let len = wire_length(Point::new(88.9, 63.5), Point::new(114.3, 63.5));
        assert!((len - 25.4).abs() < 1e-9);
        assert!((wire_length(Point::new(0.0, 0.0), Point::new(3.0, 4.0)) - 5.0).abs() < 1e-12);
    }

    #[test]
    fn only_y_fields_go_through_the_page_origin() {
        assert!(PositionField::Y.is_y() && PositionField::EndY.is_y());
        assert!(!PositionField::X.is_y() && !PositionField::EndX.is_y());
    }

    #[test]
    fn every_grid_cell_names_a_different_anchor() {
        let mut names = Vec::new();
        for v in ANCHOR_ROWS {
            for h in ANCHOR_COLUMNS {
                names.push(anchor_name(h, v));
            }
        }
        let mut unique = names.clone();
        unique.sort_by_key(|id| id.as_str());
        unique.dedup();
        assert_eq!(unique.len(), 9);
        assert_eq!(
            anchor_name(HAlign::Left, VAlign::Bottom),
            ids::PROPERTIES_ANCHOR_BOTTOM_LEFT
        );
    }

    #[test]
    fn rotation_marks_the_quarter_turn_it_rounds_to() {
        assert_eq!(rotation_step(89.9999), Some(90.0));
        assert_eq!(rotation_step(0.0), Some(0.0));
        assert_eq!(rotation_step(-90.0), Some(270.0));
        assert_eq!(rotation_step(360.0), Some(0.0));
        assert_eq!(rotation_step(45.0), None);
    }
}
