//! #1273: a partner's zone is read on its own sheet's paper and frame.

use signex_types::schematic::{Point, SchematicSheet};

use super::zone_of;

#[test]
fn a_zone_is_read_on_the_sheets_own_paper_and_zone_counts() {
    let mut sheet = SchematicSheet {
        paper_size: "A4".into(),
        ..SchematicSheet::default()
    };
    let at = Point::new(100.0, 50.0);
    // A4 landscape, 8 x 6 zones in the 277 x 190 mm drawing area.
    assert_eq!(zone_of(&sheet, at).as_deref(), Some("B3"));

    // 4 x 2 zones: 69.25 mm columns, 95 mm rows.
    sheet.frame.horizontal_zones = 4;
    sheet.frame.vertical_zones = 2;
    assert_eq!(zone_of(&sheet, at).as_deref(), Some("A2"));

    sheet.frame.show_border = false;
    assert_eq!(zone_of(&sheet, at), None);
}
