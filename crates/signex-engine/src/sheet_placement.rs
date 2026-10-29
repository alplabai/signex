//! Building a new hierarchical sheet symbol from a drag.
//!
//! Pure domain rules for the Place Sheet tool: the minimum size a drag must
//! reach, the corner normalisation, and the unique `sheetN` default for the
//! name and file. The tool only collects the two corners and the names
//! already in use.

use std::path::Path;

use signex_types::schematic::{ChildSheet, FillType, Point};

/// Shortest edge, in millimetres, a placed sheet symbol may have. A stray
/// click (zero or near-zero area) falls under this and is rejected.
pub const MIN_CHILD_SHEET_EDGE_MM: f64 = 5.08;

/// Outline width of a freshly placed sheet symbol.
pub const DEFAULT_CHILD_SHEET_STROKE_MM: f64 = 0.15;

/// Extension of a child sheet's file.
const SHEET_EXTENSION: &str = "snxsch";

/// First `sheetN` (N from 1) whose stem is not in `taken`.
///
/// `taken` mixes sheet names and file names: a file name contributes its
/// stem (`sub/sheet2.snxsch` -> `sheet2`), a bare name contributes itself.
/// Comparison ignores ASCII case, since the project may live on a
/// case-insensitive file system.
pub fn unused_sheet_stem<'a>(taken: impl IntoIterator<Item = &'a str>) -> String {
    let used: Vec<String> = taken
        .into_iter()
        .map(|entry| {
            Path::new(entry)
                .file_stem()
                .map_or_else(|| entry.to_string(), |s| s.to_string_lossy().into_owned())
                .to_ascii_lowercase()
        })
        .collect();
    (1u32..)
        .map(|n| format!("sheet{n}"))
        .find(|candidate| !used.contains(candidate))
        .unwrap_or_else(|| "sheet".to_string())
}

/// Build a sheet symbol spanning the two dragged corners, named `stem` with
/// file `stem.snxsch`. Returns `None` when either edge is shorter than
/// [`MIN_CHILD_SHEET_EDGE_MM`].
///
/// The name is the file stem so qualified net names never start with `/`.
/// The child file is not created here; a missing child renders as missing.
pub fn child_sheet_from_corners(a: Point, b: Point, stem: &str) -> Option<ChildSheet> {
    let (width, height) = ((a.x - b.x).abs(), (a.y - b.y).abs());
    if width < MIN_CHILD_SHEET_EDGE_MM || height < MIN_CHILD_SHEET_EDGE_MM {
        return None;
    }
    Some(ChildSheet {
        uuid: uuid::Uuid::now_v7(),
        name: stem.to_string(),
        filename: format!("{stem}.{SHEET_EXTENSION}"),
        position: Point::new(a.x.min(b.x), a.y.min(b.y)),
        size: (width, height),
        stroke_width: DEFAULT_CHILD_SHEET_STROKE_MM,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stem_skips_names_and_file_stems_in_use() {
        let taken = ["sheet1.snxsch", "Sheet2", "sub/sheet3.snxsch", "other"];
        assert_eq!(unused_sheet_stem(taken), "sheet4");
        assert_eq!(unused_sheet_stem([]), "sheet1");
    }

    #[test]
    fn corners_normalise_and_default_to_the_stem() {
        let s = child_sheet_from_corners(Point::new(30.0, 40.0), Point::new(10.0, 20.0), "sheet1")
            .expect("big enough");
        assert_eq!((s.position.x, s.position.y), (10.0, 20.0));
        assert_eq!(s.size, (20.0, 20.0));
        assert_eq!(s.name, "sheet1");
        assert_eq!(s.filename, "sheet1.snxsch");
        assert!(s.pins.is_empty());
    }

    #[test]
    fn sub_minimum_or_zero_area_drags_are_rejected() {
        let o = Point::new(0.0, 0.0);
        assert!(child_sheet_from_corners(o, o, "sheet1").is_none());
        assert!(child_sheet_from_corners(o, Point::new(50.0, 1.0), "sheet1").is_none());
        assert!(child_sheet_from_corners(o, Point::new(1.0, 50.0), "sheet1").is_none());
    }
}
