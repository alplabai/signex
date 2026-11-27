//! The frame zone a point of the sheet lies in (#1273), named as the zone
//! strip labels it: row letter then column number, `B3`.
//!
//! Same grid as `frame::push_zones`: the drawing area (the sheet inset by
//! the border margin) split into `horizontal_zones` columns numbered from the
//! left and `vertical_zones` rows lettered from the top.

use super::Frame;
use super::frame::zone_letter;

/// The zone of point `(x, y)` (sheet mm, top-left origin, Y down) on a
/// `width_mm` × `height_mm` sheet drawn with `frame`. A point past the
/// drawing area takes the nearest zone. `None` when the frame draws no zone
/// strip: border or zone markers off, no margin, or both counts zero. With
/// one count zero the name is only the other part (`3`, `B`).
pub fn zone_at(width_mm: f64, height_mm: f64, frame: &Frame, x: f64, y: f64) -> Option<String> {
    let margin = frame.border_margin_mm.max(0.0);
    let inner_w = width_mm - 2.0 * margin;
    let inner_h = height_mm - 2.0 * margin;
    let drawn = frame.show_border && frame.show_zone_markers && margin > 0.0;
    if !drawn || inner_w <= 0.0 || inner_h <= 0.0 || !x.is_finite() || !y.is_finite() {
        return None;
    }
    let column = cell(x - margin, inner_w, frame.horizontal_zones).map(|i| (i + 1).to_string());
    let row = cell(y - margin, inner_h, frame.vertical_zones).map(zone_letter);
    match (row, column) {
        (None, None) => None,
        (row, column) => Some(format!(
            "{}{}",
            row.unwrap_or_default(),
            column.unwrap_or_default()
        )),
    }
}

/// The 0-based cell `offset` falls in when `extent` is split into `count`
/// equal cells, clamped to the first and last; `None` with no cells.
fn cell(offset: f64, extent: f64, count: u8) -> Option<usize> {
    let count = usize::from(count);
    if count == 0 {
        return None;
    }
    let step = extent / count as f64;
    let index = (offset / step).floor();
    Some(if index < 0.0 {
        0
    } else {
        (index as usize).min(count - 1)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(columns: u8, rows: u8) -> Frame {
        Frame {
            horizontal_zones: columns,
            vertical_zones: rows,
            ..Frame::default()
        }
    }

    /// A4 landscape, 10 mm margin, 8 x 6 zones: each column is 34.625 mm,
    /// each row 31.667 mm, counted from the drawing area's top-left corner.
    #[test]
    fn a_point_is_named_by_row_letter_then_column_number() {
        let f = frame(8, 6);
        assert_eq!(zone_at(297.0, 210.0, &f, 11.0, 11.0).as_deref(), Some("A1"));
        assert_eq!(
            zone_at(297.0, 210.0, &f, 100.0, 50.0).as_deref(),
            Some("B3")
        );
        assert_eq!(
            zone_at(297.0, 210.0, &f, 286.0, 199.0).as_deref(),
            Some("F8")
        );
    }

    #[test]
    fn a_point_past_the_drawing_area_takes_the_nearest_zone() {
        let f = frame(8, 6);
        assert_eq!(zone_at(297.0, 210.0, &f, -40.0, 2.0).as_deref(), Some("A1"));
        assert_eq!(
            zone_at(297.0, 210.0, &f, 500.0, 500.0).as_deref(),
            Some("F8")
        );
    }

    #[test]
    fn no_zone_strip_names_no_zone() {
        let mut f = frame(8, 6);
        f.show_border = false;
        assert_eq!(zone_at(297.0, 210.0, &f, 50.0, 50.0), None);
        let mut f = frame(8, 6);
        f.show_zone_markers = false;
        assert_eq!(zone_at(297.0, 210.0, &f, 50.0, 50.0), None);
        assert_eq!(zone_at(297.0, 210.0, &frame(0, 0), 50.0, 50.0), None);
        assert_eq!(zone_at(297.0, 210.0, &frame(8, 6), f64::NAN, 50.0), None);
    }

    #[test]
    fn with_one_count_zero_only_the_other_part_is_named() {
        assert_eq!(
            zone_at(297.0, 210.0, &frame(4, 0), 100.0, 50.0).as_deref(),
            Some("2")
        );
        assert_eq!(
            zone_at(297.0, 210.0, &frame(0, 3), 100.0, 50.0).as_deref(),
            Some("A")
        );
    }

    /// The zone strip labels the same cells: the label drawn at a cell's
    /// centre names the zone `zone_at` gives for that centre.
    #[test]
    fn the_names_match_the_zone_strip() {
        let f = frame(4, 3);
        let strip = super::super::lower_sheet_frame(297.0, 210.0, &f, None, |t| t.to_string());
        let column_x: Vec<f64> = strip
            .iter()
            .filter_map(|p| match p {
                super::super::FramePrimitive::Text { x, text, .. } if text == "3" => Some(*x),
                _ => None,
            })
            .collect();
        let row_y: Vec<f64> = strip
            .iter()
            .filter_map(|p| match p {
                super::super::FramePrimitive::Text { y, text, .. } if text == "B" => Some(*y),
                _ => None,
            })
            .collect();
        assert!(!column_x.is_empty() && !row_y.is_empty());
        assert_eq!(
            zone_at(297.0, 210.0, &f, column_x[0], row_y[0]).as_deref(),
            Some("B3")
        );
    }
}
