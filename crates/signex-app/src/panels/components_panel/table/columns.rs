//! What the table knows about a column besides its values: its caption,
//! its default width and which side its text sits on.

use signex_i18n::{I18n, ids};
use signex_library::ParamKind;
use signex_library::templates::{ColumnKey, ParameterTemplate};

use crate::library::range_filter::parameter_caption;

/// The caption a column header, the Columns picker and a group chip show.
/// A parameter column reads its name with a capital first letter.
pub fn caption(column: &ColumnKey, i18n: &I18n) -> String {
    let id = match column {
        ColumnKey::PartNumber => ids::PANELS_COMPONENTS_COLUMN_PART_NUMBER,
        ColumnKey::Description => ids::PANELS_COMPONENTS_COLUMN_DESCRIPTION,
        ColumnKey::Manufacturer => ids::PANELS_COMPONENTS_COLUMN_MANUFACTURER,
        ColumnKey::LibraryRef => ids::PANELS_COMPONENTS_COLUMN_LIBRARY_REF,
        ColumnKey::Footprint => ids::PANELS_COMPONENTS_COLUMN_FOOTPRINT,
        ColumnKey::Stock => ids::PANELS_COMPONENTS_COLUMN_STOCK,
        ColumnKey::Price => ids::PANELS_COMPONENTS_COLUMN_PRICE,
        ColumnKey::Param(name) => return parameter_caption(i18n, name),
    };
    i18n.message_plain(id, &[])
}

/// The width a column opens at before the user drags it, in logical
/// pixels (the mockup's per-kind widths).
pub fn default_width(column: &ColumnKey, template: Option<&ParameterTemplate>) -> f32 {
    match column {
        ColumnKey::PartNumber => 158.0,
        ColumnKey::Description => 220.0,
        ColumnKey::Manufacturer => 128.0,
        ColumnKey::LibraryRef => 100.0,
        ColumnKey::Footprint => 170.0,
        ColumnKey::Stock => 66.0,
        ColumnKey::Price => 62.0,
        ColumnKey::Param(name) => match template {
            Some(t) if t.range(name).is_some() => 112.0,
            Some(t)
                if t.slot(name)
                    .is_some_and(|s| s.kind == ParamKind::Measurement) =>
            {
                84.0
            }
            _ => 92.0,
        },
    }
}

/// Numbers sit right: stock and price.
pub fn is_right_aligned(column: &ColumnKey) -> bool {
    matches!(column, ColumnKey::Stock | ColumnKey::Price)
}

/// Whether a column can be a group level: every column but part number
/// and description, whose values are unique per part (the mockup's "+
/// Group" menu leaves them out).
pub fn is_groupable(column: &ColumnKey) -> bool {
    !matches!(column, ColumnKey::PartNumber | ColumnKey::Description)
}
